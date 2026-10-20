use super::*;
use converter_core::{dsl::DslReader, formats::DictionaryReader};

struct Candidate {
    file: PreparedFile,
    options: ReadOptions,
    title: Option<String>,
    existing: Option<(String, String)>,
    incoming: Option<usize>,
    members: BTreeSet<PathBuf>,
    frozen: bool,
}
fn pending(state: JobState) -> bool {
    !matches!(
        state,
        JobState::Succeeded
            | JobState::SucceededWithWarnings
            | JobState::Failed
            | JobState::Cancelled
            | JobState::Interrupted
    )
}
fn inspect(mut candidate: Candidate, ctx: &RunContext) -> Candidate {
    let path = candidate
        .file
        .path
        .canonicalize()
        .unwrap_or_else(|_| candidate.file.path.clone());
    candidate.members.insert(path);
    if candidate.frozen {
        return candidate;
    }
    let lower = candidate.file.display_name.to_lowercase();
    if (lower.ends_with(".dsl") || lower.ends_with(".dsl.dz"))
        && let Ok(bundles) =
            prepare_source_with_options(&candidate.file, &[], ctx, &candidate.options)
        && let Some(bundle) = bundles.first()
    {
        for file in &bundle.includes {
            candidate.members.insert(file.path.clone());
        }
        if candidate.title.is_none()
            && let Ok(metadata) = DslReader.inspect(bundle, &candidate.options, ctx)
        {
            candidate.title = Some(metadata.title);
        }
    }
    candidate
}
impl JobManager {
    pub async fn create_batch(&self, drafts: Vec<JobDraft>) -> AppResult<CreateJobsResult> {
        let _submission = self.inner.submissions.lock().await;
        if drafts.len() > Limits::default().max_archive_entries {
            return Err(
                AppError::new("archive_limit", "Слишком много файлов источников")
                    .with_message("messages.archive_limit", message_args([])),
            );
        }
        let mut candidates = Vec::new();
        for (index, draft) in drafts.iter().enumerate() {
            let RegisteredSource::Local(file) = self.register(&draft.source)? else {
                return Err(AppError::new(
                    "invalid_source",
                    "Для группового выбора нужны локальные файлы",
                )
                .with_message("messages.invalid_source", message_args([])));
            };
            for resource in &draft.resources {
                self.register(resource)?;
            }
            candidates.push(Candidate {
                file,
                options: draft.options.clone(),
                title: draft
                    .metadata_override
                    .as_ref()
                    .map(|meta| meta.title.clone())
                    .filter(|title| !title.trim().is_empty()),
                existing: None,
                incoming: Some(index),
                members: BTreeSet::new(),
                frozen: false,
            });
        }
        {
            let runtime = self.inner.runtime.lock().map_err(|_| lock_error())?;
            for job in runtime
                .jobs
                .values()
                .filter(|job| pending(job.snapshot.state))
            {
                if let Some(RegisteredSource::Local(file)) = job.sources.first() {
                    candidates.push(Candidate {
                        file: file.clone(),
                        options: job.draft.options.clone(),
                        title: job
                            .snapshot
                            .metadata
                            .as_ref()
                            .map(|meta| meta.title.clone())
                            .filter(|title| !title.trim().is_empty()),
                        existing: Some((job.snapshot.id.clone(), job.snapshot.run_id.clone())),
                        incoming: None,
                        members: job
                            .prepared_sources
                            .as_ref()
                            .map(|paths| paths.iter().cloned().collect())
                            .unwrap_or_default(),
                        frozen: job.prepared_sources.is_some() || job.ready.is_some(),
                    });
                }
            }
        }
        let scratch_root = self.inner.store.root.join("scratch");
        let mut candidates = tokio::task::spawn_blocking(move || -> AppResult<_> {
            fs::create_dir_all(&scratch_root)?;
            let scratch = tempfile::Builder::new()
                .prefix("selection-")
                .tempdir_in(scratch_root)?;
            let ctx = RunContext::new(scratch.path().to_owned());
            Ok(candidates
                .into_iter()
                .map(|candidate| inspect(candidate, &ctx))
                .collect::<Vec<_>>())
        })
        .await
        .map_err(|_| {
            AppError::new("worker_failed", "Не удалось проверить выбранные файлы")
                .with_message("messages.worker_failed", message_args([]))
        })??;
        let live = |candidates: &mut [Candidate]| -> AppResult<_> {
            let runtime = self.inner.runtime.lock().map_err(|_| lock_error())?;
            for candidate in candidates {
                if let Some((id, run)) = &candidate.existing
                    && let Some(job) = runtime.jobs.get(id)
                    && job.snapshot.run_id == *run
                {
                    refresh_members(
                        candidate,
                        job.prepared_sources.as_deref(),
                        job.ready.is_some(),
                    );
                    if let Some(metadata) = &job.snapshot.metadata
                        && !metadata.title.trim().is_empty()
                    {
                        candidate.title = Some(metadata.title.clone());
                    }
                }
            }
            Ok(runtime
                .jobs
                .iter()
                .map(|(id, job)| {
                    (
                        id.clone(),
                        (job.snapshot.run_id.clone(), job.snapshot.state),
                    )
                })
                .collect::<BTreeMap<_, _>>())
        };
        let current = live(&mut candidates)?;
        let owners = ownership(&candidates, &current);
        let mut skipped: BTreeMap<usize, BTreeSet<String>> = BTreeMap::new();
        for candidate in &candidates {
            let Some((id, run)) = &candidate.existing else {
                continue;
            };
            let path = candidate
                .file
                .path
                .canonicalize()
                .unwrap_or_else(|_| candidate.file.path.clone());
            let Some(&owner) = owners.get(&path) else {
                continue;
            };
            if candidates[owner].incoming.is_some()
                && self
                    .job(id)
                    .is_ok_and(|job| job.snapshot.run_id == *run && pending(job.snapshot.state))
            {
                self.cancel(id).await?;
                self.wait_idle(id).await?;
                skipped
                    .entry(owner)
                    .or_default()
                    .insert(candidate.file.display_name.clone());
            }
        }
        // Отмена частей могла уступить поток. Повторно читаем живое состояние:
        // отменённый/удалённый/перезапущенный основной процесс не поглощает новый выбор.
        let current = live(&mut candidates)?;
        let owners = ownership(&candidates, &current);
        let mut keep = BTreeSet::new();
        for (index, candidate) in candidates.iter().enumerate() {
            let Some(incoming) = candidate.incoming else {
                continue;
            };
            let path = candidate
                .file
                .path
                .canonicalize()
                .unwrap_or_else(|_| candidate.file.path.clone());
            let owner = *owners
                .get(&path)
                .expect("выбранный источник имеет владельца");
            if owner == index {
                keep.insert(incoming);
            } else {
                skipped
                    .entry(owner)
                    .or_default()
                    .insert(candidate.file.display_name.clone());
            }
        }
        let mut jobs = Vec::new();
        for (index, draft) in drafts.into_iter().enumerate() {
            if keep.contains(&index) {
                jobs.push(self.create_unchecked(draft).await?);
            }
        }
        let included = skipped
            .into_iter()
            .map(|(owner, names)| IncludedDictionaryNotice {
                main_name: candidates[owner]
                    .title
                    .clone()
                    .unwrap_or_else(|| candidates[owner].file.display_name.clone()),
                dictionary_names: names.into_iter().collect(),
            })
            .collect();
        Ok(CreateJobsResult { jobs, included })
    }
}

fn refresh_members(candidate: &mut Candidate, prepared: Option<&[PathBuf]>, ready: bool) {
    if prepared.is_none() && !ready && !candidate.frozen {
        return;
    }
    candidate.members = prepared
        .map(|paths| paths.iter().cloned().collect())
        .unwrap_or_default();
    candidate.members.insert(
        candidate
            .file
            .path
            .canonicalize()
            .unwrap_or_else(|_| candidate.file.path.clone()),
    );
    candidate.frozen = prepared.is_some() || ready;
}

fn ownership(
    candidates: &[Candidate],
    live: &BTreeMap<String, (String, JobState)>,
) -> BTreeMap<PathBuf, usize> {
    let mut order: Vec<_> = (0..candidates.len())
        .filter(|&index| {
            candidates[index].existing.as_ref().is_none_or(|(id, run)| {
                live.get(id).is_some_and(|(current, state)| {
                    current == run && (pending(*state) || state::success(*state))
                })
            })
        })
        .collect();
    order.sort_by_key(|&index| {
        (
            std::cmp::Reverse(candidates[index].members.len()),
            candidates[index].existing.is_none(),
            index,
        )
    });
    let mut owners = BTreeMap::new();
    for index in order {
        let candidate = &candidates[index];
        let path = candidate
            .file
            .path
            .canonicalize()
            .unwrap_or_else(|_| candidate.file.path.clone());
        if owners.contains_key(&path) {
            continue;
        }
        for member in &candidate.members {
            owners.entry(member.clone()).or_insert(index);
        }
    }
    owners
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn владелец_проверяется_по_состоянию_после_анализа_а_не_снимку_до_него() {
        let part = PathBuf::from("/selected/part.dsl");
        let main = PathBuf::from("/selected/main.dsl");
        let candidates = [
            Candidate {
                file: PreparedFile {
                    id: "part".into(),
                    display_name: "part.dsl".into(),
                    path: part.clone(),
                },
                options: ReadOptions::default(),
                title: None,
                existing: None,
                incoming: Some(0),
                members: BTreeSet::from([part.clone()]),
                frozen: false,
            },
            Candidate {
                file: PreparedFile {
                    id: "main".into(),
                    display_name: "main.dsl".into(),
                    path: main.clone(),
                },
                options: ReadOptions::default(),
                title: None,
                existing: Some(("main-job".into(), "old-run".into())),
                incoming: None,
                members: BTreeSet::from([main, part.clone()]),
                frozen: false,
            },
        ];
        let active =
            BTreeMap::from([("main-job".into(), ("old-run".into(), JobState::Converting))]);
        assert_eq!(ownership(&candidates, &active)[&part], 1);
        for state in [JobState::Cancelled, JobState::Failed, JobState::Interrupted] {
            let after = BTreeMap::from([("main-job".into(), ("old-run".into(), state))]);
            assert_eq!(
                ownership(&candidates, &after)[&part],
                0,
                "новый выбор не должен исчезать после {state:?}"
            );
        }
        assert_eq!(ownership(&candidates, &BTreeMap::new())[&part], 0);
        let restarted =
            BTreeMap::from([("main-job".into(), ("new-run".into(), JobState::Converting))]);
        assert_eq!(ownership(&candidates, &restarted)[&part], 0);
    }
    #[test]
    fn состав_успевший_зафиксироваться_заменяет_предварительный_анализ() {
        let main = PathBuf::from("/selected/main.dsl");
        let part = PathBuf::from("/selected/part.dsl");
        let mut candidate = Candidate {
            file: PreparedFile {
                id: "main".into(),
                display_name: "main.dsl".into(),
                path: main.clone(),
            },
            options: ReadOptions::default(),
            title: None,
            existing: Some(("job".into(), "run".into())),
            incoming: None,
            members: BTreeSet::from([main.clone(), part.clone()]),
            frozen: false,
        };
        // На диске уже появилось новое включение, но работающий reader
        // успел подготовить прежний состав без этой части.
        refresh_members(&mut candidate, Some(std::slice::from_ref(&main)), false);
        assert_eq!(candidate.members, BTreeSet::from([main.clone()]));
        refresh_members(&mut candidate, Some(&[main.clone(), part]), true);
        assert_eq!(candidate.members.len(), 2);
        // Уточнение параметров потребовало пересоздать артефакты.
        refresh_members(&mut candidate, None, false);
        assert_eq!(candidate.members, BTreeSet::from([main]));
        assert!(!candidate.frozen);
    }
}
