//! Docker container monitoring provider.
//!
//! Connects to the local Docker daemon via `bollard`, polls container
//! listings and stats, and writes results to the shared store.
//! Gracefully handles Docker not being available.

use std::time::Duration;

use bollard::container::{ListContainersOptions, StatsOptions, StopContainerOptions, StartContainerOptions, RestartContainerOptions, KillContainerOptions};
use bollard::Docker;
use futures_util::TryStreamExt;

use crate::core::state::{DockerContainer, DockerAction};
use crate::core::store::Store;

/// Interval between Docker polling cycles.
const DOCKER_POLL_INTERVAL: Duration = Duration::from_secs(3);

/// Spawn a long-running Docker monitoring task.
pub async fn watch_docker(store: Store) {
    // Attempt to connect to the Docker daemon
    let docker = match Docker::connect_with_local_defaults() {
        Ok(d) => d,
        Err(e) => {
            let mut st = store.write().await;
            st.docker_available = false;
            st.docker_error = Some(format!("Cannot connect to Docker: {}", e));
            return;
        }
    };

    // Verify the connection actually works by pinging
    match docker.ping().await {
        Ok(_) => {
            store.write().await.docker_available = true;
        }
        Err(e) => {
            let mut st = store.write().await;
            st.docker_available = false;
            st.docker_error = Some(format!("Docker ping failed: {}", e));
            return;
        }
    }

    // Main polling loop
    loop {
        // Check for confirmed actions
        let action_to_perform = {
            let mut st = store.write().await;
            st.docker_action_confirmed.take()
        };

        if let Some((action, container_id)) = action_to_perform {
            if let Err(e) = execute_docker_action(&docker, action, &container_id).await {
                let mut st = store.write().await;
                st.docker_error = Some(format!("Action failed: {}", e));
            }
        }

        if let Err(e) = poll_containers(&docker, &store).await {
            let mut st = store.write().await;
            st.docker_error = Some(format!("Docker error: {}", e));
            // Don't return — keep retrying
        }

        tokio::time::sleep(DOCKER_POLL_INTERVAL).await;
    }
}

/// Execute a Docker action.
pub async fn execute_docker_action(docker: &Docker, action: DockerAction, container_id: &str) -> Result<(), String> {
    match action {
        DockerAction::Stop => {
            docker.stop_container(container_id, None::<StopContainerOptions>)
                .await
                .map_err(|e| format!("Stop failed: {}", e))
        }
        DockerAction::Start => {
            docker.start_container(container_id, None::<StartContainerOptions<String>>)
                .await
                .map_err(|e| format!("Start failed: {}", e))
        }
        DockerAction::Restart => {
            docker.restart_container(container_id, None::<RestartContainerOptions>)
                .await
                .map_err(|e| format!("Restart failed: {}", e))
        }
        DockerAction::Kill => {
            docker.kill_container(container_id, None::<KillContainerOptions<String>>)
                .await
                .map_err(|e| format!("Kill failed: {}", e))
        }
    }
}

/// Poll all containers and their stats, updating the store.
async fn poll_containers(docker: &Docker, store: &Store) -> Result<(), String> {
    // List ALL containers (including stopped)
    let opts = ListContainersOptions::<String> {
        all: true,
        ..Default::default()
    };

    let container_list = docker
        .list_containers(Some(opts))
        .await
        .map_err(|e| format!("Failed to list containers: {}", e))?;

    let mut results: Vec<DockerContainer> = Vec::with_capacity(container_list.len());

    // Optimization: fetch stats in parallel
    let mut tasks = Vec::with_capacity(container_list.len());

    for container in &container_list {
        let id = match &container.id {
            Some(id) => id.clone(),
            None => continue,
        };

        let name = container
            .names
            .as_ref()
            .and_then(|n| n.first())
            .map(|n| n.trim_start_matches('/').to_string())
            .unwrap_or_else(|| id[..12].to_string());

        let image = container
            .image
            .clone()
            .unwrap_or_else(|| "<none>".to_string());

        let status = container
            .status
            .clone()
            .unwrap_or_else(|| "Unknown".to_string());

        let state = container
            .state
            .clone()
            .unwrap_or_else(|| "unknown".to_string());

        let docker_c = docker.clone();
        let id_c = id.clone();
        
        tasks.push(async move {
            let stats = if state == "running" {
                fetch_container_stats(&docker_c, &id_c).await.unwrap_or_default()
            } else {
                (0.0, 0, 0, 0, 0)
            };
            (id_c, name, image, status, state, stats)
        });
    }

    let joined_results = futures_util::future::join_all(tasks).await;

    for (id, name, image, status, state, (cpu_percent, mem_usage, mem_limit, net_rx, net_tx)) in joined_results {
        results.push(DockerContainer {
            id,
            name,
            image,
            status,
            state,
            cpu_percent,
            mem_usage,
            mem_limit,
            net_rx,
            net_tx,
        });
    }

    // Write to store
    let mut st = store.write().await;
    st.containers = results;
    st.docker_available = true;
    st.docker_error = None;

    Ok(())
}

/// Fetch a single one-shot stats snapshot for a container.
/// Returns (cpu_percent, mem_usage, mem_limit, net_rx, net_tx).
async fn fetch_container_stats(
    docker: &Docker,
    container_id: &str,
) -> Result<(f64, u64, u64, u64, u64), String> {
    let opts = StatsOptions {
        stream: false,
        one_shot: true,
    };

    let stats = docker
        .stats(container_id, Some(opts))
        .try_next()
        .await
        .map_err(|e| format!("Stats error: {}", e))?
        .ok_or_else(|| "No stats returned".to_string())?;

    // CPU percentage calculation
    let cpu_percent = calculate_cpu_percent(&stats);

    // Memory
    let mem_usage = stats.memory_stats.usage.unwrap_or(0);
    let mem_limit = stats.memory_stats.limit.unwrap_or(0);

    // Network I/O (sum across all interfaces)
    let (mut net_rx, mut net_tx) = (0u64, 0u64);
    if let Some(networks) = &stats.networks {
        for net_stats in networks.values() {
            net_rx = net_rx.saturating_add(net_stats.rx_bytes);
            net_tx = net_tx.saturating_add(net_stats.tx_bytes);
        }
    }

    Ok((cpu_percent, mem_usage, mem_limit, net_rx, net_tx))
}

/// Calculate CPU usage percentage from a stats snapshot.
fn calculate_cpu_percent(stats: &bollard::container::Stats) -> f64 {
    let cpu_delta = stats.cpu_stats.cpu_usage.total_usage as f64
        - stats.precpu_stats.cpu_usage.total_usage as f64;

    let system_delta = stats.cpu_stats.system_cpu_usage.unwrap_or(0) as f64
        - stats.precpu_stats.system_cpu_usage.unwrap_or(0) as f64;

    if system_delta > 0.0 && cpu_delta >= 0.0 {
        (cpu_delta / system_delta) * 100.0
    } else {
        0.0
    }
}
