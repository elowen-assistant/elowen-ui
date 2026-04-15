pub(super) fn job_count_label(count: usize) -> String {
    format!("{count} jobs")
}

pub(super) fn short_thread_label(thread_id: &str) -> String {
    if thread_id.len() > 8 {
        thread_id[..8].to_string()
    } else {
        thread_id.to_string()
    }
}
