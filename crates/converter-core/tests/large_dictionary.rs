#[path = "support/large_fixture.rs"]
mod fixture;

use converter_core::{model::*, pipeline::convert_to_staging};
use mdictlib::{ChecksumPolicy, MdxFile, OpenOptions};
use std::{
    sync::{
        Arc,
        atomic::{AtomicBool, AtomicU64, Ordering},
    },
    thread::{self, JoinHandle},
    time::{Duration, Instant},
};
use sysinfo::{Pid, ProcessRefreshKind, ProcessesToUpdate, System};

struct RssSampler {
    stop: Arc<AtomicBool>,
    peak: Arc<AtomicU64>,
    worker: Option<JoinHandle<()>>,
}
impl RssSampler {
    fn start() -> Self {
        let stop = Arc::new(AtomicBool::new(false));
        let peak = Arc::new(AtomicU64::new(0));
        let worker_stop = stop.clone();
        let worker_peak = peak.clone();
        let worker = thread::spawn(move || {
            let mut system = System::new();
            let pid = Pid::from_u32(std::process::id());
            while !worker_stop.load(Ordering::Relaxed) {
                system.refresh_processes_specifics(
                    ProcessesToUpdate::Some(&[pid]),
                    false,
                    ProcessRefreshKind::nothing().with_memory(),
                );
                if let Some(process) = system.process(pid) {
                    worker_peak.fetch_max(process.memory(), Ordering::Relaxed);
                }
                thread::sleep(Duration::from_millis(100));
            }
        });
        Self {
            stop,
            peak,
            worker: Some(worker),
        }
    }
    fn finish(mut self) -> u64 {
        self.stop.store(true, Ordering::Relaxed);
        if let Some(worker) = self.worker.take() {
            worker.join().expect("Сбор RSS завершился с ошибкой");
        }
        self.peak.load(Ordering::Relaxed)
    }
}
impl Drop for RssSampler {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::Relaxed);
        if let Some(worker) = self.worker.take() {
            let _ = worker.join();
        }
    }
}

#[test]
#[ignore = "Генерирует больше 64 MiB вне Git и измеряет RSS реального процесса"]
fn крупный_словарь_с_ограниченным_буфером() -> std::result::Result<(), Box<dyn std::error::Error>> {
    let temp = tempfile::tempdir()?;
    let source = temp.path().join("large.dsl");
    let fixture = fixture::generate(&source, 1_000_000)?;
    let mut ctx = RunContext::new(temp.path().join("work"));
    ctx.limits.merge_fan_in = 2;
    assert!(fixture.bytes > ctx.limits.sort_buffer_bytes as u64);
    ctx.progress = Arc::new(|event| {
        if let CoreEvent::StageCompleted(progress) = event {
            println!(
                "Этап {:?}: {} {:?}",
                progress.stage, progress.completed, progress.unit
            );
        }
    });
    let request = ConversionRequest {
        bundle: InputBundle {
            annotation: None,
            includes: vec![],
            main: PreparedFile {
                id: "large".into(),
                display_name: "large.dsl".into(),
                path: source,
            },
            abbreviations: None,
            resources: vec![],
        },
        options: ReadOptions::default(),
        metadata_override: None,
        staging_dir: temp.path().join("ready"),
    };
    println!(
        "Вход: {} байт; {} статей; sort budget: {} байт",
        fixture.bytes, fixture.articles, ctx.limits.sort_buffer_bytes
    );
    let sampler = RssSampler::start();
    let start = Instant::now();
    let artifacts = convert_to_staging(&request, &ctx)?;
    let elapsed = start.elapsed();
    let peak = sampler.finish();
    assert_eq!(artifacts.summary.source_articles, fixture.articles);
    assert_eq!(artifacts.summary.output_keys, 3 * fixture.articles);
    assert!(artifacts.summary.sort_metrics.merge_passes >= 2);
    assert!(artifacts.summary.sort_metrics.peak_sort_bytes <= ctx.limits.sort_buffer_bytes);
    assert_eq!(artifacts.summary.warnings, 0);
    assert!(peak > 0, "Не удалось измерить RSS процесса");
    let dictionary = MdxFile::open_with_options(
        &artifacts.mdx,
        &OpenOptions::new().with_checksum_policy(ChecksumPolicy::Verify),
    )?;
    assert!(dictionary.lookup("word00000000")?.is_some());
    assert!(dictionary.lookup("word00999999")?.is_some());
    println!(
        "Результат: {:.3} с; sampled peak RSS: {} байт (интервал 100 мс); merge passes: {}; peak sort rows: {} байт; MDX: {} байт",
        elapsed.as_secs_f64(),
        peak,
        artifacts.summary.sort_metrics.merge_passes,
        artifacts.summary.sort_metrics.peak_sort_bytes,
        std::fs::metadata(&artifacts.mdx)?.len()
    );
    Ok(())
}
