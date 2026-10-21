use crate::{
    engine::{self, Plan, PreparedCandidate, Scan, Selection},
    store::{durable_json, now, private_directory, Store},
    CoreError, Result,
};
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Operation {
    pub version: u32,
    pub id: String,
    pub plan_id: String,
    pub repository_path: String,
    pub state: String,
    pub created_at: String,
    pub updated_at: String,
    pub prepared: Option<PreparedCandidate>,
    pub error: Option<String>,
}

#[derive(Clone)]
pub struct Application {
    pub store: Store,
}

impl Application {
    pub fn open(root: &Path) -> Result<Self> {
        Ok(Self {
            store: Store::open(root)?,
        })
    }

    pub fn scan(&self, path: &Path) -> Result<Scan> {
        let scan = engine::scan(path)?;
        self.store.put("scan", &scan.id, &scan)?;
        self.store.put("scan_time", &scan.id, &now())?;
        Ok(scan)
    }

    pub fn plan(&self, scan_id: &str, selection: &Selection) -> Result<Plan> {
        let scan: Scan = self.store.get("scan", scan_id)?;
        let plan = engine::plan(&scan, selection)?;
        self.store.put("plan", &plan.id, &plan)?;
        Ok(plan)
    }

    pub fn create_demo(&self) -> Result<PathBuf> {
        let root = self.store.root().join("demos");
        private_directory(&root)?;
        let path = root.join(uuid::Uuid::new_v4().to_string());
        engine::create_demo(&path)?;
        Ok(path)
    }

    pub fn operation(&self, id: &str) -> Result<Operation> {
        self.store.operation_dir(id)?;
        let operation: Operation = self.store.get("operation", id)?;
        if operation.id != id {
            return Err(CoreError::blocked(
                "Operation record identity mismatch; artifacts were preserved.",
            ));
        }
        if operation.version != 1 {
            return Err(CoreError::blocked(
                "Unsupported operation version; artifacts were preserved.",
            ));
        }
        Ok(operation)
    }

    pub fn operations(&self) -> Result<Vec<Operation>> {
        let operations: Vec<Operation> = self.store.list("operation")?;
        if operations.iter().any(|operation| operation.version != 1) {
            return Err(CoreError::blocked(
                "Unsupported operation version; artifacts and journal states were preserved.",
            ));
        }
        Ok(operations)
    }

    fn checkpoint(&self, operation: &mut Operation, state: &str) -> Result<()> {
        operation.state = state.into();
        operation.updated_at = now();
        self.store.checkpoint(&operation.id, state, operation)
    }

    pub fn prepare(
        &self,
        plan_id: &str,
        mut progress: impl FnMut(&Operation),
    ) -> Result<Operation> {
        let plan: Plan = self.store.get("plan", plan_id)?;
        if !plan.blockers.is_empty() {
            return Err(CoreError::blocked(plan.blockers.join("; ")));
        }
        let _lock = self.store.repository_lock(&plan.scan.source_path)?;
        self.check_repository_available(&plan.scan.source_path)?;
        let id = uuid::Uuid::new_v4().to_string();
        let root = self.store.operation_dir(&id)?;
        private_directory(&root)?;
        durable_json(&root.join("plan.json"), &plan)?;
        let mut operation = Operation {
            version: 1,
            id,
            plan_id: plan.id.clone(),
            repository_path: plan.scan.source_path.clone(),
            state: "PLANNED".into(),
            created_at: now(),
            updated_at: now(),
            prepared: None,
            error: None,
        };
        self.checkpoint(&mut operation, "PLANNED")?;
        progress(&operation);
        // The engine owns a fresh child directory; journal artifacts never enter the candidate.
        let result = engine::prepare_with_progress(&plan, &root.join("artifacts"), |state| {
            if self
                .store
                .get::<bool>("cancel", &operation.id)
                .unwrap_or(false)
            {
                return Err(CoreError::new(
                    "CANCELLED_BEFORE_PUSH",
                    "Local preparation cancelled. Existing artifacts were retained.",
                ));
            }
            // Attach the durable prepared manifest before claiming VALIDATED in the journal.
            if state != "VALIDATED" {
                self.checkpoint(&mut operation, state)?;
                progress(&operation);
            }
            Ok(())
        });
        match result {
            Ok(prepared) => {
                durable_json(&root.join("prepared.json"), &prepared)?;
                let passed = prepared.validation.passed;
                operation.prepared = Some(prepared);
                self.checkpoint(&mut operation, if passed { "VALIDATED" } else { "BLOCKED" })?;
                progress(&operation);
                Ok(operation)
            }
            Err(error) => {
                operation.error = Some(error.message.clone());
                let state = if error.code == "CANCELLED_BEFORE_PUSH" {
                    "CANCELLED_BEFORE_PUSH"
                } else {
                    "FAILED_LOCAL"
                };
                self.checkpoint(&mut operation, state)?;
                progress(&operation);
                Err(error)
            }
        }
    }

    fn check_repository_available(&self, path: &str) -> Result<()> {
        for operation in self.operations()? {
            if operation.repository_path == path
                && matches!(
                    operation.state.as_str(),
                    "PLANNED"
                        | "BACKUP_VERIFIED"
                        | "PREPARING"
                        | "PUBLISHING"
                        | "REMOTE_OUTCOME_UNKNOWN"
                        | "REMOTE_DRIFT"
                        | "NEEDS_ATTENTION"
                )
            {
                return Err(CoreError::blocked(format!(
                    "Resolve operation {} before preparing another candidate.",
                    operation.id
                )));
            }
        }
        Ok(())
    }

    pub fn verify(&self, id: &str) -> Result<Operation> {
        let mut operation = self.operation(id)?;
        if !matches!(
            operation.state.as_str(),
            "VALIDATED" | "BLOCKED" | "FAILED_LOCAL"
        ) {
            return Err(CoreError::blocked(
                "Local verification cannot change an active, unknown, or remote operation state.",
            ));
        }
        let _lock = self.store.repository_lock(&operation.repository_path)?;
        let plan: Plan = self.store.get("plan", &operation.plan_id)?;
        let prepared = operation.prepared.as_mut().ok_or_else(|| {
            CoreError::blocked(
                "No complete candidate is available; prepare a new reviewed local plan.",
            )
        })?;
        if Path::new(&prepared.operation_dir) != self.store.operation_dir(id)?.join("artifacts") {
            operation.error = Some(
                "Candidate storage does not belong to this operation; artifacts were preserved."
                    .into(),
            );
            self.checkpoint(&mut operation, "BLOCKED")?;
            return Err(CoreError::blocked(
                "Candidate storage does not belong to this operation",
            ));
        }
        prepared.validation = match engine::verify(&plan, prepared) {
            Ok(report) => report,
            Err(error) => {
                operation.error = Some(error.message.clone());
                self.checkpoint(&mut operation, "BLOCKED")?;
                return Err(error);
            }
        };
        let state = if prepared.validation.passed {
            "VALIDATED"
        } else {
            "BLOCKED"
        };
        durable_json(
            &self.store.operation_dir(id)?.join("prepared.json"),
            prepared,
        )?;
        operation.error = if state == "VALIDATED" {
            None
        } else {
            Some("Candidate failed local revalidation; inspect the failed checks.".into())
        };
        self.checkpoint(&mut operation, state)?;
        Ok(operation)
    }

    pub fn cancel(&self, id: &str) -> Result<()> {
        let operation = self.operation(id)?;
        if !matches!(
            operation.state.as_str(),
            "PLANNED" | "BACKUP_VERIFIED" | "PREPARING"
        ) {
            return Err(CoreError::blocked(
                "This operation is not preparing local artifacts.",
            ));
        }
        self.store.put("cancel", id, &true)
    }

    pub fn recover(&self) -> Result<()> {
        for mut operation in self.operations()? {
            if !matches!(
                operation.state.as_str(),
                "PLANNED"
                    | "BACKUP_VERIFIED"
                    | "PREPARING"
                    | "PUBLISHING"
                    | "REMOTE_OUTCOME_UNKNOWN"
            ) {
                continue;
            }
            let Ok(_lock) = self.store.repository_lock(&operation.repository_path) else {
                continue;
            };
            if matches!(
                operation.state.as_str(),
                "PUBLISHING" | "REMOTE_OUTCOME_UNKNOWN"
            ) {
                operation.error = Some("Publication requires remote reconciliation. Inspector cannot resolve or retry a remote write.".into());
                self.checkpoint(&mut operation, "REMOTE_OUTCOME_UNKNOWN")?;
            } else {
                operation.error = Some("Local preparation was interrupted. Artifacts and backups were retained; a fresh local preparation is required.".into());
                self.checkpoint(&mut operation, "FAILED_LOCAL")?;
            }
        }
        Ok(())
    }

    pub fn report_preview(&self, id: &str) -> Result<serde_json::Value> {
        let operation = self.operation(id)?;
        let plan: Plan = self.store.get("plan", &operation.plan_id)?;
        let checks = operation
            .prepared
            .as_ref()
            .map(|p| {
                p.validation
                    .checks
                    .iter()
                    .map(|c| serde_json::json!({"name":c.name,"passed":c.passed}))
                    .collect::<Vec<_>>()
            })
            .unwrap_or_default();
        let preview = serde_json::json!({
            "schema_version":1,"operation_id":id,"state":operation.state,
            "created_at":operation.created_at,"exported_at":now(),
            "selected_commits":plan.selected_oids.len(),"affected_commits":plan.affected_oids.len(),
            "validation_checks":checks,"journal":self.store.events(id)?,
            "publication":"Remote publication and reconciliation are unavailable in this Inspector. The recorded operation state is authoritative; this report does not independently verify remote refs.",
            "calendar":"Not observed for this local operation.",
            "redaction":"Repository names, paths, identities, emails, commit IDs, messages, source files, and credentials omitted."
        });
        self.store.put("report_preview", id, &preview)?;
        Ok(preview)
    }

    pub fn export_report(&self, id: &str) -> Result<PathBuf> {
        let path = self.store.operation_dir(id)?.join("support-report.json");
        let preview: serde_json::Value = self
            .store
            .get("report_preview", id)
            .map_err(|_| CoreError::blocked("Preview the redacted report before exporting it."))?;
        durable_json(&path, &preview)?;
        Ok(path)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn recovery_preserves_unknown_remote_and_skips_live_work() {
        let temp = tempfile::tempdir().unwrap();
        let app = Application::open(temp.path()).unwrap();
        let mut op = Operation {
            version: 1,
            id: uuid::Uuid::new_v4().to_string(),
            plan_id: "plan".into(),
            repository_path: "/test".into(),
            state: "PLANNED".into(),
            created_at: now(),
            updated_at: now(),
            prepared: None,
            error: None,
        };
        app.checkpoint(&mut op, "PLANNED").unwrap();
        let lock = app.store.repository_lock("/test").unwrap();
        app.recover().unwrap();
        assert_eq!(app.operation(&op.id).unwrap().state, "PLANNED");
        drop(lock);
        app.recover().unwrap();
        assert_eq!(app.operation(&op.id).unwrap().state, "FAILED_LOCAL");
        app.checkpoint(&mut op, "PUBLISHING").unwrap();
        app.recover().unwrap();
        assert_eq!(
            app.operation(&op.id).unwrap().state,
            "REMOTE_OUTCOME_UNKNOWN"
        );
        assert!(app.check_repository_available("/test").is_err());
    }
}
