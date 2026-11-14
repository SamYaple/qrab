//! Bounded, volatile asynchronous operation tracking. No cancellation is
//! advertised: ending monitoring never claims that a guest operation stopped.
use super::*;
use std::{collections::BTreeMap, sync::Mutex, time::Instant};

pub(super) const SERVICE: &str = "/redfish/v1/TaskService";
pub(super) const TASKS: &str = "/redfish/v1/TaskService/Tasks";
pub(super) const START_UPDATE: &str =
    "/redfish/v1/TaskService/Actions/Oem/QrabTaskService.StartGuestUpdate";
pub(super) const CAPACITY: usize = 32;
const ACTIVE_LIMIT: usize = 2;
const RETENTION: Duration = Duration::from_secs(3600);
const MONITOR_TIMEOUT: Duration = Duration::from_secs(300);
const OUTPUT_LIMIT: usize = 4096;
pub(super) const ATTACHMENT_LIMIT: usize = 128 * 1024;

#[derive(Clone, Default)]
pub struct TaskStore(Arc<Mutex<Registry>>);

#[derive(Default)]
struct Registry {
    next: u64,
    records: BTreeMap<u64, Record>,
}

#[derive(Clone)]
pub(super) struct Record {
    pub id: u64,
    pub name: &'static str,
    target: &'static str,
    start_ms: u64,
    end_ms: Option<u64>,
    finished: Option<Instant>,
    pub outcome: Option<Outcome>,
}

#[derive(Clone)]
pub(super) enum Outcome {
    Completed { output: Option<String> },
    Diagnostic { bytes: Bytes },
    Failed { message: &'static str },
}

fn now_ms() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis()
        .try_into()
        .unwrap_or(u64::MAX)
}

fn task_uri(id: u64) -> String {
    format!("{TASKS}/{id}")
}
fn monitor_uri(id: u64) -> String {
    format!("{SERVICE}/TaskMonitors/{id}")
}

fn parse_id(id: &str) -> Result<u64, Failure> {
    id.parse::<u64>()
        .ok()
        .filter(|n| n.to_string() == id && *n != 0)
        .ok_or_else(Failure::not_found)
}

impl Registry {
    fn prune(&mut self) {
        self.records
            .retain(|_, r| r.finished.is_none_or(|at| at.elapsed() < RETENTION));
    }
}

impl TaskStore {
    pub(super) fn begin(&self, name: &'static str, target: &'static str) -> Result<u64, Failure> {
        let mut registry = self.0.lock().unwrap();
        registry.prune();
        if registry
            .records
            .values()
            .filter(|r| r.outcome.is_none())
            .count()
            >= ACTIVE_LIMIT
        {
            return Err(Failure::unavailable(
                "The asynchronous operation limit has been reached.",
            ));
        }
        if registry.records.len() >= CAPACITY {
            // Active tasks are never evicted to admit another operation.
            let oldest = registry
                .records
                .iter()
                .find(|(_, r)| r.outcome.is_some())
                .map(|(id, _)| *id);
            if let Some(id) = oldest {
                registry.records.remove(&id);
            } else {
                return Err(Failure::unavailable("The task registry is full."));
            }
        }
        registry.next = registry
            .next
            .checked_add(1)
            .ok_or_else(|| Failure::unavailable("The task ID range is exhausted."))?;
        let id = registry.next;
        registry.records.insert(
            id,
            Record {
                id,
                name,
                target,
                start_ms: now_ms(),
                end_ms: None,
                finished: None,
                outcome: None,
            },
        );
        Ok(id)
    }

    pub(super) fn finish(&self, id: u64, outcome: Outcome) {
        let outcome = match outcome {
            Outcome::Completed {
                output: Some(output),
            } => Outcome::Completed {
                output: Some(tail(&output, OUTPUT_LIMIT)),
            },
            Outcome::Diagnostic { ref bytes } if bytes.len() > ATTACHMENT_LIMIT => {
                Outcome::Failed {
                    message: "The diagnostic bundle exceeds its byte limit.",
                }
            }
            outcome => outcome,
        };
        let mut registry = self.0.lock().unwrap();
        if let Some(record) = registry.records.get_mut(&id) {
            if record.outcome.is_none() {
                record.end_ms = Some(now_ms());
                record.finished = Some(Instant::now());
                record.outcome = Some(outcome);
            }
        }
    }

    pub(super) fn get(&self, id: u64) -> Option<Record> {
        let mut registry = self.0.lock().unwrap();
        registry.prune();
        registry.records.get(&id).cloned()
    }

    #[cfg(test)]
    fn list(&self) -> Vec<Record> {
        let mut registry = self.0.lock().unwrap();
        registry.prune();
        registry.records.values().cloned().collect()
    }

    pub(super) fn diagnostics(&self) -> Vec<Record> {
        let mut registry = self.0.lock().unwrap();
        registry.prune();
        registry
            .records
            .values()
            .filter(|r| matches!(r.outcome, Some(Outcome::Diagnostic { .. })))
            .cloned()
            .collect()
    }

    fn links(&self) -> Vec<Value> {
        let mut registry = self.0.lock().unwrap();
        registry.prune();
        registry
            .records
            .keys()
            .map(|id| link(&task_uri(*id)))
            .collect()
    }

    pub(super) fn stats(&self) -> Value {
        let mut registry = self.0.lock().unwrap();
        registry.prune();
        let mut attachments = 0usize;
        let mut outputs = 0usize;
        let mut active = 0usize;
        for record in registry.records.values() {
            match &record.outcome {
                None => active += 1,
                Some(Outcome::Diagnostic { bytes }) => attachments += bytes.len(),
                Some(Outcome::Completed {
                    output: Some(output),
                }) => outputs += output.len(),
                _ => {}
            }
        }
        json!({"RetainedTasks":registry.records.len(),"ActiveTasks":active,"AttachmentBytes":attachments,"OutputBytes":outputs,"MaxTasks":CAPACITY,"MaxActiveTasks":ACTIVE_LIMIT})
    }
}

impl Record {
    pub(super) fn json(&self, admin: bool) -> Value {
        let mut task = json!({
            "@odata.id":task_uri(self.id),"@odata.type":"#Task.v1_7_4.Task",
            "Id":self.id.to_string(),"Name":self.name,"StartTime":rfc3339(self.start_ms),
            "TaskMonitor":monitor_uri(self.id),"TaskState":"Running",
            "HidePayload":true,
            "Oem":{"Qrab":{"@odata.type":"#QrabTask.v1_0_0.QrabTask","TargetUri":self.target}}
        });
        match &self.outcome {
            None => {}
            Some(outcome) => {
                task["EndTime"] = json!(rfc3339(self.end_ms.unwrap_or(self.start_ms)));
                task["TaskState"] = json!(if matches!(outcome, Outcome::Failed { .. }) {
                    "Exception"
                } else {
                    "Completed"
                });
                task["TaskStatus"] = json!(if matches!(outcome, Outcome::Failed { .. }) {
                    "Critical"
                } else {
                    "OK"
                });
                match outcome {
                    Outcome::Completed {
                        output: Some(output),
                    } if admin => task["Oem"]["Qrab"]["Output"] = json!(output),
                    Outcome::Diagnostic { .. } => {
                        task["Links"] = json!({"CreatedResources":[link(&super::diagnostics::entry_uri(self.id))]})
                    }
                    Outcome::Failed { message } => {
                        task["Messages"] = json!([{
                            "@odata.type":"#Message.v1_2_0.Message","MessageId":"Base.1.18.GeneralError",
                            "Message":message,"Severity":"Critical","Resolution":"Inspect VM health and retry when appropriate."
                        }])
                    }
                    _ => {}
                }
            }
        }
        task
    }
}

pub(super) fn accepted(store: &TaskStore, id: u64) -> Response {
    let task = store.get(id).expect("new task exists");
    (
        StatusCode::ACCEPTED,
        [
            (header::LOCATION, monitor_uri(id)),
            (header::RETRY_AFTER, "1".into()),
        ],
        Json(task.json(true)),
    )
        .into_response()
}

pub(super) async fn service(
    State(app): State<AppState>,
    headers: HeaderMap,
) -> Result<Json<Value>, Failure> {
    principal(&app, &headers).await?;
    Ok(Json(json!({
        "@odata.id":SERVICE,"@odata.type":"#TaskService.v1_2_1.TaskService",
        "Id":"TaskService","Name":"VM asynchronous operations","ServiceEnabled":true,
        "CompletedTaskOverWritePolicy":"Oldest","TaskAutoDeleteTimeoutMinutes":60,
        "LifeCycleEventOnTaskStateChange":false,"Tasks":link(TASKS),
        "Actions":{"Oem":{"#QrabTaskService.StartGuestUpdate":{"target":START_UPDATE}}},
        "Oem":{"Qrab":{"@odata.type":"#QrabTaskService.v1_0_0.QrabTaskService",
            "MaxTasks":CAPACITY,"MaxActiveTasks":ACTIVE_LIMIT,"Persistent":false,"MaxAttachmentBytes":ATTACHMENT_LIMIT}}
    })))
}

pub(super) async fn collection_handler(
    State(app): State<AppState>,
    headers: HeaderMap,
) -> Result<Json<Value>, Failure> {
    principal(&app, &headers).await?;
    Ok(Json(collection(
        TASKS,
        "TaskCollection",
        "VM tasks",
        app.tasks.links(),
    )))
}

pub(super) async fn task(
    State(app): State<AppState>,
    headers: HeaderMap,
    Path(id): Path<String>,
) -> Result<Json<Value>, Failure> {
    let who = principal(&app, &headers).await?;
    let task = app
        .tasks
        .get(parse_id(&id)?)
        .ok_or_else(Failure::not_found)?;
    Ok(Json(task.json(who.role == Role::Admin)))
}

pub(super) async fn monitor(
    State(app): State<AppState>,
    headers: HeaderMap,
    Path(id): Path<String>,
) -> Result<Response, Failure> {
    let who = principal(&app, &headers).await?;
    let task = app
        .tasks
        .get(parse_id(&id)?)
        .ok_or_else(Failure::not_found)?;
    Ok(match &task.outcome {
        None => (StatusCode::ACCEPTED, [(header::RETRY_AFTER,"1")], Json(task.json(who.role == Role::Admin))).into_response(),
        Some(Outcome::Completed { .. }) => StatusCode::NO_CONTENT.into_response(),
        Some(Outcome::Diagnostic { .. }) => (StatusCode::CREATED, [(header::LOCATION, super::diagnostics::entry_uri(task.id))], Json(super::diagnostics::entry_json(&task))).into_response(),
        Some(Outcome::Failed { message }) => Failure::new(StatusCode::INTERNAL_SERVER_ERROR,"GeneralError",*message,"Inspect VM health before retrying; an unobserved guest operation might still be running.").into_response(),
    })
}

fn tail(text: &str, limit: usize) -> String {
    let mut start = text.len().saturating_sub(limit);
    while !text.is_char_boundary(start) {
        start += 1;
    }
    text[start..].into()
}

pub(super) async fn start_update(
    State(app): State<AppState>,
    headers: HeaderMap,
    body: Bytes,
) -> Result<Response, Failure> {
    let who = admin(&app, &headers).await?;
    let body = action_body(&body)?;
    let args = string_list(&body, "Args")?.unwrap_or_default();
    if body
        .as_object()
        .is_some_and(|map| map.keys().any(|key| key != "Args"))
    {
        return Err(Failure::new(
            StatusCode::BAD_REQUEST,
            "ActionParameterUnknown",
            "The action only accepts Args.",
            "Remove unknown parameters.",
        ));
    }
    // Mirror the manager's validation before reserving or dispatching a task.
    if args.len() > 16
        || args.iter().any(|arg| {
            arg.is_empty()
                || arg.len() > 256
                || arg.starts_with('-')
                || !arg
                    .bytes()
                    .all(|b| b.is_ascii_alphanumeric() || b"._:/=+@,-".contains(&b))
        })
    {
        return Err(Failure::new(
            StatusCode::BAD_REQUEST,
            "ActionParameterValueFormatError",
            "Invalid guest update arguments.",
            "Use at most 16 arguments of 1..256 allowed ASCII characters, without a leading '-'.",
        ));
    }
    let id = app.tasks.begin("Guest update", START_UPDATE)?;
    let response = accepted(&app.tasks, id);
    let store = app.tasks.clone();
    let control = app.control.clone();
    tracing::info!(subject=%who.subject, task_id=id, "Redfish asynchronous guest update requested");
    tokio::spawn(async move {
        let work =
            async {
                let started = control.agent_update_trigger(args).await.map_err(|_| {
                    "The guest update could not be started or its outcome is unknown."
                })?;
                let run_id = started.run_id;
                let mut status = started;
                loop {
                    if status.run_id != run_id {
                        return Err("The guest update run changed before its result was observed.");
                    }
                    match status.state {
                        pod::UpdateState::Succeeded => {
                            return Ok(Outcome::Completed {
                                output: Some(tail(&status.output, OUTPUT_LIMIT)),
                            })
                        }
                        pod::UpdateState::Failed => return Err(
                            "The guest update failed; inspect the guest update status for details.",
                        ),
                        pod::UpdateState::Idle => {
                            return Err("The guest update did not enter a running state.")
                        }
                        pod::UpdateState::Running => {}
                    }
                    tokio::time::sleep(Duration::from_secs(1)).await;
                    status = control.agent_update_status().await.map_err(|_| {
                        "The guest update status is unavailable; the update may still be running."
                    })?;
                }
            };
        let outcome = match tokio::time::timeout(MONITOR_TIMEOUT, work).await {
            Ok(Ok(outcome)) => outcome,
            Ok(Err(message)) => Outcome::Failed { message },
            Err(_) => Outcome::Failed {
                message:
                    "Guest update monitoring timed out; the guest operation may still be running.",
            },
        };
        store.finish(id, outcome);
    });
    Ok(response)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn active_tasks_are_bounded_and_never_evicted() {
        let store = TaskStore::default();
        let first = store.begin("first", START_UPDATE).unwrap();
        let second = store.begin("second", START_UPDATE).unwrap();
        assert!(store.begin("third", START_UPDATE).is_err());
        store.finish(second, Outcome::Completed { output: None });
        for _ in 0..CAPACITY + 3 {
            let id = store.begin("done", START_UPDATE).unwrap();
            store.finish(id, Outcome::Completed { output: None });
        }
        assert!(store.get(first).unwrap().outcome.is_none());
        assert_eq!(store.list().len(), CAPACITY);
        assert!(store.get(second).is_none());
    }

    #[test]
    fn tasks_redact_output_and_do_not_claim_progress_or_cancellation() {
        let store = TaskStore::default();
        let id = store.begin("update", START_UPDATE).unwrap();
        let running = store.get(id).unwrap().json(false);
        assert_eq!(running["TaskState"], "Running");
        assert!(running.get("TaskStatus").is_none() && running.get("EndTime").is_none());
        assert!(running.get("PercentComplete").is_none() && running.get("Actions").is_none());
        store.finish(
            id,
            Outcome::Completed {
                output: Some("private-output".into()),
            },
        );
        let record = store.get(id).unwrap();
        assert!(!record.json(false).to_string().contains("private-output"));
        assert!(record.json(true).to_string().contains("private-output"));
        assert_eq!(record.json(false)["TaskStatus"], "OK");
        store.finish(
            id,
            Outcome::Failed {
                message: "late failure",
            },
        );
        assert_eq!(store.get(id).unwrap().json(false)["TaskState"], "Completed");
    }

    #[test]
    fn expired_terminal_tasks_are_removed_but_active_tasks_remain() {
        let store = TaskStore::default();
        let active = store.begin("active", START_UPDATE).unwrap();
        let expired = store.begin("expired", START_UPDATE).unwrap();
        store.finish(expired, Outcome::Completed { output: None });
        store
            .0
            .lock()
            .unwrap()
            .records
            .get_mut(&expired)
            .unwrap()
            .finished = Some(Instant::now() - RETENTION);
        assert!(store.get(expired).is_none());
        assert!(store.get(active).is_some());
        assert!(parse_id("01").is_err() && parse_id("0").is_err());
        assert_eq!(tail("αβγ", 3), "γ");
    }
}
