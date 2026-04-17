use codex_protocol::protocol::RolloutItem;
use codex_protocol::protocol::ThreadMemoryMode;
use codex_rollout::StateDbHandle;
use codex_rollout::append_rollout_item_to_path;
use codex_rollout::find_archived_thread_path_by_id_str;
use codex_rollout::find_thread_path_by_id_str;
use codex_rollout::read_session_meta_line;

use super::LocalThreadStore;
use crate::ReadThreadParams;
use crate::SetThreadMemoryModeParams;
use crate::StoredThread;
use crate::ThreadStoreError;
use crate::ThreadStoreResult;
use crate::local::read_thread;

pub(super) async fn set_thread_memory_mode(
    store: &LocalThreadStore,
    params: SetThreadMemoryModeParams,
) -> ThreadStoreResult<StoredThread> {
    let thread_id = params.thread_id;
    let rollout_path = resolve_rollout_path(store, thread_id, params.include_archived).await?;
    let mut session_meta = read_session_meta_line(rollout_path.as_path())
        .await
        .map_err(|err| ThreadStoreError::Internal {
            message: format!("failed to set thread memory mode: {err}"),
        })?;
    if session_meta.meta.id != thread_id {
        return Err(ThreadStoreError::Internal {
            message: format!(
                "failed to set thread memory mode: rollout session metadata id mismatch: expected {thread_id}, found {}",
                session_meta.meta.id
            ),
        });
    }

    session_meta.meta.memory_mode = Some(memory_mode_as_str(params.memory_mode).to_string());
    let item = RolloutItem::SessionMeta(session_meta);
    append_rollout_item_to_path(rollout_path.as_path(), &item)
        .await
        .map_err(|err| ThreadStoreError::Internal {
            message: format!("failed to set thread memory mode: {err}"),
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

fn memory_mode_as_str(mode: ThreadMemoryMode) -> &'static str {
    match mode {
        ThreadMemoryMode::Enabled => "enabled",
        ThreadMemoryMode::Disabled => "disabled",
    }
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
    async fn set_thread_memory_mode_updates_active_rollout() {
        let home = TempDir::new().expect("temp dir");
        let store = LocalThreadStore::new(test_config(home.path()));
        let uuid = Uuid::from_u128(302);
        let thread_id = ThreadId::from_string(&uuid.to_string()).expect("valid thread id");
        let path =
            write_session_file(home.path(), "2025-01-03T14-30-00", uuid).expect("session file");

        let thread = store
            .set_thread_memory_mode(SetThreadMemoryModeParams {
                thread_id,
                memory_mode: ThreadMemoryMode::Disabled,
                include_archived: false,
            })
            .await
            .expect("set thread memory mode");

        assert_eq!(thread.thread_id, thread_id);
        let last_line = std::fs::read_to_string(path)
            .expect("read rollout")
            .lines()
            .last()
            .expect("last line")
            .to_string();
        let appended: Value = serde_json::from_str(&last_line).expect("json line");
        assert_eq!(appended["type"], "session_meta");
        assert_eq!(appended["payload"]["id"], thread_id.to_string());
        assert_eq!(appended["payload"]["memory_mode"], "disabled");
    }

    #[tokio::test]
    async fn set_thread_memory_mode_rejects_mismatched_session_meta_id() {
        let home = TempDir::new().expect("temp dir");
        let store = LocalThreadStore::new(test_config(home.path()));
        let filename_uuid = Uuid::from_u128(303);
        let metadata_uuid = Uuid::from_u128(304);
        let thread_id = ThreadId::from_string(&filename_uuid.to_string()).expect("valid thread id");
        let path = write_session_file(home.path(), "2025-01-03T15-00-00", filename_uuid)
            .expect("session file");
        let content = std::fs::read_to_string(&path).expect("read rollout");
        std::fs::write(
            &path,
            content.replace(&filename_uuid.to_string(), &metadata_uuid.to_string()),
        )
        .expect("rewrite rollout");

        let err = store
            .set_thread_memory_mode(SetThreadMemoryModeParams {
                thread_id,
                memory_mode: ThreadMemoryMode::Enabled,
                include_archived: false,
            })
            .await
            .expect_err("mismatch should fail");

        assert!(matches!(err, ThreadStoreError::Internal { .. }));
        assert!(err.to_string().contains("metadata id mismatch"));
    }
}
