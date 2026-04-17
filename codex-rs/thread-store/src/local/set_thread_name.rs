use codex_protocol::protocol::EventMsg;
use codex_protocol::protocol::RolloutItem;
use codex_protocol::protocol::ThreadNameUpdatedEvent;
use codex_rollout::StateDbHandle;
use codex_rollout::append_rollout_item_to_path;
use codex_rollout::append_thread_name;
use codex_rollout::find_archived_thread_path_by_id_str;
use codex_rollout::find_thread_path_by_id_str;

use super::LocalThreadStore;
use crate::ReadThreadParams;
use crate::SetThreadNameParams;
use crate::StoredThread;
use crate::ThreadStoreError;
use crate::ThreadStoreResult;
use crate::local::read_thread;

pub(super) async fn set_thread_name(
    store: &LocalThreadStore,
    params: SetThreadNameParams,
) -> ThreadStoreResult<StoredThread> {
    let thread_id = params.thread_id;
    let rollout_path = resolve_rollout_path(store, thread_id, params.include_archived).await?;
    let item = RolloutItem::EventMsg(EventMsg::ThreadNameUpdated(ThreadNameUpdatedEvent {
        thread_id,
        thread_name: Some(params.name.clone()),
    }));

    append_rollout_item_to_path(rollout_path.as_path(), &item)
        .await
        .map_err(|err| ThreadStoreError::Internal {
            message: format!("failed to set thread name: {err}"),
        })?;
    append_thread_name(store.config.codex_home.as_path(), thread_id, &params.name)
        .await
        .map_err(|err| ThreadStoreError::Internal {
            message: format!("failed to index thread name: {err}"),
        })?;

    let state_db_ctx = open_state_db_for_direct_thread_lookup(store).await;
    codex_rollout::state_db::reconcile_rollout(
        state_db_ctx.as_deref(),
        rollout_path.as_path(),
        store.config.model_provider_id.as_str(),
        /*builder*/ None,
        &[],
        /*archived_only*/ None,
        /*new_thread_memory_mode*/ None,
    )
    .await;

    read_thread::read_thread(
        store,
        ReadThreadParams {
            thread_id,
            include_archived: params.include_archived,
            include_history: false,
        },
    )
    .await
}

async fn open_state_db_for_direct_thread_lookup(store: &LocalThreadStore) -> Option<StateDbHandle> {
    codex_state::StateRuntime::init(
        store.config.sqlite_home.clone(),
        store.config.model_provider_id.clone(),
    )
    .await
    .ok()
}

async fn resolve_rollout_path(
    store: &LocalThreadStore,
    thread_id: codex_protocol::ThreadId,
    include_archived: bool,
) -> ThreadStoreResult<std::path::PathBuf> {
    let active_path =
        find_thread_path_by_id_str(store.config.codex_home.as_path(), &thread_id.to_string())
            .await
            .map_err(|err| ThreadStoreError::InvalidRequest {
                message: format!("failed to locate thread id {thread_id}: {err}"),
            })?;
    if let Some(path) = active_path {
        return Ok(path);
    }
    if !include_archived {
        return Err(ThreadStoreError::InvalidRequest {
            message: format!("thread not found: {thread_id}"),
        });
    }
    find_archived_thread_path_by_id_str(store.config.codex_home.as_path(), &thread_id.to_string())
        .await
        .map_err(|err| ThreadStoreError::InvalidRequest {
            message: format!("failed to locate archived thread id {thread_id}: {err}"),
        })?
        .ok_or_else(|| ThreadStoreError::InvalidRequest {
            message: format!("thread not found: {thread_id}"),
        })
}

#[cfg(test)]
mod tests {
    use codex_protocol::ThreadId;
    use pretty_assertions::assert_eq;
    use serde_json::Value;
    use tempfile::TempDir;
    use uuid::Uuid;

    use super::*;
    use crate::ThreadStore;
    use crate::local::LocalThreadStore;
    use crate::local::test_support::test_config;
    use crate::local::test_support::write_session_file;

    #[tokio::test]
    async fn set_thread_name_updates_active_rollout_and_indexes_name() {
        let home = TempDir::new().expect("temp dir");
        let store = LocalThreadStore::new(test_config(home.path()));
        let uuid = Uuid::from_u128(301);
        let thread_id = ThreadId::from_string(&uuid.to_string()).expect("valid thread id");
        let path =
            write_session_file(home.path(), "2025-01-03T14-00-00", uuid).expect("session file");

        let thread = store
            .set_thread_name(SetThreadNameParams {
                thread_id,
                name: "A sharper name".to_string(),
                include_archived: false,
            })
            .await
            .expect("set thread name");

        assert_eq!(thread.name.as_deref(), Some("A sharper name"));
        let latest_name = codex_rollout::find_thread_name_by_id(home.path(), &thread_id)
            .await
            .expect("find thread name");
        assert_eq!(latest_name.as_deref(), Some("A sharper name"));

        let last_line = std::fs::read_to_string(path)
            .expect("read rollout")
            .lines()
            .last()
            .expect("last line")
            .to_string();
        let appended: Value = serde_json::from_str(&last_line).expect("json line");
        assert_eq!(appended["type"], "event_msg");
        assert_eq!(appended["payload"]["type"], "thread_name_updated");
        assert_eq!(appended["payload"]["thread_id"], thread_id.to_string());
        assert_eq!(appended["payload"]["thread_name"], "A sharper name");
    }
}
