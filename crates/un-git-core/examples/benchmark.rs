//! Synthetic local benchmarks only; never points at a user repository.
use std::{
    io::Write,
    process::{Command, Stdio},
    time::Instant,
};
use un_git_core::engine::{self, Selection};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let root = tempfile::tempdir()?;
    let mut measurements = Vec::new();
    for count in [1_000usize, 10_000, 100_000] {
        let repo = root.path().join(format!("fixture-{count}.git"));
        let status = Command::new("git")
            .args([
                "-c",
                "init.templateDir=",
                "init",
                "--quiet",
                "--bare",
                "--initial-branch=main",
            ])
            .arg(&repo)
            .env("GIT_CONFIG_GLOBAL", "/dev/null")
            .env("GIT_CONFIG_NOSYSTEM", "1")
            .status()?;
        assert!(status.success());
        let mut child = Command::new("git")
            .arg("-C")
            .arg(&repo)
            .args(["fast-import", "--quiet"])
            .env("GIT_CONFIG_GLOBAL", "/dev/null")
            .env("GIT_CONFIG_NOSYSTEM", "1")
            .stdin(Stdio::piped())
            .stdout(Stdio::null())
            .spawn()?;
        let mut input = child.stdin.take().unwrap();
        for i in 0..count {
            let message = format!("Synthetic commit {i}\n");
            write!(input,"commit refs/heads/main\nmark :{}\nauthor Benchmark <benchmark@un-git.test> {} +0000\ncommitter Benchmark <benchmark@un-git.test> {} +0000\ndata {}\n{}",i+1,1_609_459_200+i,1_609_459_200+i,message.len(),message)?;
            if i > 0 {
                writeln!(input, "from :{i}")?;
            }
            writeln!(input)?;
        }
        drop(input);
        assert!(child.wait()?.success());
        let start = Instant::now();
        let scan = engine::scan(&repo)?;
        let scan_ms = start.elapsed().as_millis();
        let selected = scan
            .commits
            .iter()
            .find(|commit| commit.message == format!("Synthetic commit {}\n", count / 2))
            .unwrap();
        let start = Instant::now();
        let plan = engine::plan(
            &scan,
            &Selection {
                commit_oids: vec![selected.oid.clone()],
                confirmed_email: "benchmark@un-git.test".into(),
                replacement_email: engine::default_replacement_email(),
            },
        )?;
        let plan_ms = start.elapsed().as_millis();
        let prepare_ms = if count <= 10_000 {
            let start = Instant::now();
            let prepared = engine::prepare(&plan, &root.path().join(format!("operation-{count}")))?;
            assert!(prepared.validation.passed);
            Some(start.elapsed().as_millis())
        } else {
            None
        };
        let row = serde_json::json!({"commits":count,"scan_ms":scan_ms,"plan_ms":plan_ms,"prepare_ms":prepare_ms,"selected":plan.selected_oids.len(),"affected":plan.affected_oids.len(),"scan_json_bytes":serde_json::to_vec(&scan)?.len()});
        eprintln!("{row}");
        measurements.push(row);
    }
    println!("{}", serde_json::to_string_pretty(&measurements)?);
    Ok(())
}
