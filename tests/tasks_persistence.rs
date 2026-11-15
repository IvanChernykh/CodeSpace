use codespace::tasks::{self, TaskPriority};
use std::fs;
#[test]
fn task_ids_are_unique_and_concurrent_mutations_preserve_every_task() {
    let root = std::env::temp_dir().join(format!(
        "cse-task-v3-{}-{}",
        std::process::id(),
        codespace::util::now_unix_ms()
    ));
    fs::create_dir_all(&root).unwrap_or_else(|e| panic!("fixture: {e}"));
    let workers: Vec<_> = (0..8)
        .map(|worker| {
            let root = root.clone();
            std::thread::spawn(move || {
                for index in 0..12 {
                    let mut board =
                        tasks::transaction(&root).unwrap_or_else(|e| panic!("lock: {e}"));
                    board.add(
                        &format!("worker {worker} task {index}"),
                        "",
                        TaskPriority::Medium,
                        None,
                        vec![],
                    );
                    tasks::save_tasks(&root, &board).unwrap_or_else(|e| panic!("save: {e}"));
                }
            })
        })
        .collect();
    for worker in workers {
        worker.join().unwrap_or_else(|_| panic!("worker failed"));
    }
    assert_eq!(
        tasks::load_tasks_checked(&root)
            .unwrap_or_else(|e| panic!("load: {e}"))
            .tasks
            .len(),
        96
    );
    fs::write(tasks::tasks_path(&root), "{broken").unwrap_or_else(|e| panic!("fixture: {e}"));
    assert!(tasks::transaction(&root).is_err());
    assert_eq!(
        fs::read_to_string(tasks::tasks_path(&root)).unwrap_or_default(),
        "{broken"
    );
    let _ = fs::remove_dir_all(root);
}
