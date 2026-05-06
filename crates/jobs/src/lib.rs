use tokio_cron_scheduler::JobScheduler;

pub async fn scheduler() -> anyhow::Result<JobScheduler> {
    let s = JobScheduler::new().await?;
    Ok(s)
}
