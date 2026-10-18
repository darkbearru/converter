use super::*;
use crate::jobs::store::{
    AdditionalDictionariesState, AdditionalDictionaryChild, StoredAdditionalDictionaries,
};

pub(super) fn pending_offer(offer: AdditionalDictionariesOffer) -> StoredAdditionalDictionaries {
    let children = offer
        .dictionaries
        .iter()
        .map(|dictionary| AdditionalDictionaryChild {
            job_id: uuid::Uuid::new_v4().to_string(),
            dictionary: dictionary.clone(),
        })
        .collect();
    StoredAdditionalDictionaries {
        offer,
        state: AdditionalDictionariesState::Pending,
        children,
    }
}

fn additional_child(parent: &StoredJob, plan: &AdditionalDictionaryChild) -> StoredJob {
    let mut child = parent.clone();
    child.snapshot.id = plan.job_id.clone();
    child.snapshot.display_name = plan.dictionary.display_name.clone();
    child.snapshot.generation = 0;
    child.snapshot.run_id = uuid::Uuid::new_v4().to_string();
    child.snapshot.sequence = 0;
    child.snapshot.state = JobState::Queued;
    child.snapshot.had_errors = false;
    child.snapshot.conversion = ConversionJobProgress::default();
    child.snapshot.result = None;
    child.snapshot.export = ExportSnapshot::default();
    child.snapshot.error = None;
    child.snapshot.input_choices.clear();
    child.snapshot.additional_dictionaries = None;
    child.snapshot.diagnostic_count = 0;
    child.snapshot.metadata = None;
    child.snapshot.prepare_only = false;
    child.draft.prepare_only = false;
    child.draft.auto_start = false;
    child.draft.metadata_override = None;
    child.selected_main = Some(plan.dictionary.id.clone());
    child.published = None;
    child.ready = None;
    child.staging_path = None;
    child.additional_dictionaries = None;
    child
}

fn materialize(
    store: &Store,
    jobs: &mut BTreeMap<String, StoredJob>,
    parent: &StoredJob,
    recovering: bool,
) -> AppResult<()> {
    let Some(plan) = &parent.additional_dictionaries else {
        return Ok(());
    };
    for planned in &plan.children {
        if jobs.contains_key(&planned.job_id) {
            continue;
        }
        let mut child = additional_child(parent, planned);
        if recovering {
            child.snapshot.state = JobState::Interrupted;
            child.snapshot.conversion.status = ConversionStatus::Interrupted;
        }
        store.save(&child)?;
        jobs.insert(planned.job_id.clone(), child);
    }
    Ok(())
}

pub(super) fn recover(store: &Store, jobs: &mut BTreeMap<String, StoredJob>) -> AppResult<()> {
    let accepted: Vec<_> = jobs
        .values()
        .filter(|job| {
            job.additional_dictionaries
                .as_ref()
                .is_some_and(|plan| plan.state == AdditionalDictionariesState::Accepted)
        })
        .cloned()
        .collect();
    for mut parent in accepted {
        materialize(store, jobs, &parent, true)?;
        if let Some(plan) = &mut parent.additional_dictionaries {
            plan.state = AdditionalDictionariesState::Dispatched;
        }
        parent.snapshot.additional_dictionaries = None;
        parent.snapshot.sequence += 1;
        store.save(&parent)?;
        jobs.insert(parent.snapshot.id.clone(), parent);
    }
    // Между сохранением детей и запуском worker процесс мог завершиться.
    // После открытия очереди такой worker отсутствует; повтор запускает пользователь.
    let dispatched_children: BTreeSet<_> = jobs
        .values()
        .filter_map(|parent| parent.additional_dictionaries.as_ref())
        .filter(|plan| plan.state == AdditionalDictionariesState::Dispatched)
        .flat_map(|plan| plan.children.iter().map(|child| child.job_id.clone()))
        .collect();
    for id in dispatched_children {
        let Some(mut child) = jobs.get(&id).cloned() else {
            continue;
        };
        if child.snapshot.state == JobState::Queued {
            child.snapshot.state = JobState::Interrupted;
            child.snapshot.conversion.status = ConversionStatus::Interrupted;
            child.snapshot.sequence += 1;
            store.save(&child)?;
            jobs.insert(id, child);
        }
    }
    Ok(())
}

pub(super) fn has_incomplete_dispatch(job: &StoredJob) -> bool {
    job.additional_dictionaries
        .as_ref()
        .is_some_and(|plan| plan.state == AdditionalDictionariesState::Accepted)
}

pub(super) fn protected_child(jobs: &BTreeMap<String, StoredJob>, id: &str) -> bool {
    jobs.values().any(|parent| {
        has_incomplete_dispatch(parent)
            && parent
                .additional_dictionaries
                .as_ref()
                .is_some_and(|plan| plan.children.iter().any(|child| child.job_id == id))
    })
}

impl JobManager {
    pub async fn resolve_additional(
        &self,
        id: &str,
        offer_id: &str,
        convert: bool,
    ) -> AppResult<Vec<JobSnapshot>> {
        let snapshots = {
            // Решение, дочерние записи и удаление истории используют один замок.
            let mut runtime = self.inner.runtime.lock().map_err(|_| lock_error())?;
            let mut parent = runtime
                .jobs
                .get(id)
                .cloned()
                .ok_or_else(|| AppError::new("job_not_found", "Задание не найдено"))?;
            let plan = parent
                .additional_dictionaries
                .as_ref()
                .filter(|plan| plan.offer.id == offer_id)
                .ok_or_else(|| {
                    AppError::new(
                        "additional_offer_not_found",
                        "Предложение словарей уже недоступно",
                    )
                })?;
            if plan.state == AdditionalDictionariesState::Declined {
                return Ok(vec![]);
            }
            if !convert
                && matches!(
                    plan.state,
                    AdditionalDictionariesState::Accepted | AdditionalDictionariesState::Dispatched
                )
            {
                return Err(AppError::new(
                    "additional_offer_already_accepted",
                    "Согласие уже сохранено. Повторите создание словарей или закройте окно",
                ));
            }
            if plan.state == AdditionalDictionariesState::Pending {
                parent.additional_dictionaries.as_mut().expect("План").state = if convert {
                    AdditionalDictionariesState::Accepted
                } else {
                    AdditionalDictionariesState::Declined
                };
                parent.snapshot.additional_dictionaries = None;
                parent.snapshot.sequence += 1;
                // Сначала сохраняется решение с заранее назначенными UUID детей.
                self.inner.store.save(&parent)?;
                runtime.jobs.insert(id.into(), parent.clone());
                let _ = self.inner.events.send(JobEvent {
                    snapshot: parent.snapshot.clone(),
                });
                if !convert {
                    return Ok(vec![]);
                }
            }
            if has_incomplete_dispatch(&parent) {
                materialize(&self.inner.store, &mut runtime.jobs, &parent, false)?;
                parent.additional_dictionaries.as_mut().expect("План").state =
                    AdditionalDictionariesState::Dispatched;
                parent.snapshot.sequence += 1;
                self.inner.store.save(&parent)?;
                runtime.jobs.insert(id.into(), parent.clone());
                let _ = self.inner.events.send(JobEvent {
                    snapshot: parent.snapshot.clone(),
                });
            }
            parent
                .additional_dictionaries
                .as_ref()
                .expect("План")
                .children
                .iter()
                // Удалённая после dispatch история не создаётся повторно.
                .filter_map(|child| runtime.jobs.get(&child.job_id))
                .map(|child| child.snapshot.clone())
                .collect::<Vec<_>>()
        };
        let mut result = Vec::with_capacity(snapshots.len());
        for snapshot in snapshots {
            let _ = self.inner.events.send(JobEvent {
                snapshot: snapshot.clone(),
            });
            if snapshot.state == JobState::Queued && snapshot.generation == 0 {
                match self.start_job(&snapshot.id, true).await {
                    Ok(started) => result.push(started),
                    Err(error) if error.code == "job_busy" => {
                        result.push(self.job(&snapshot.id)?.snapshot);
                    }
                    Err(error) => return Err(error),
                }
            } else {
                result.push(snapshot);
            }
        }
        Ok(result)
    }
}
