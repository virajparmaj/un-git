use serde_json::{json, Value};
use std::path::Path;
use un_git_core::{
    engine::{Plan, Scan},
    github::ImportedRepository,
    operations::{Application, Operation},
    Result,
};

pub fn repository_name(path: &str) -> String {
    Path::new(path)
        .file_name()
        .unwrap_or_default()
        .to_string_lossy()
        .into_owned()
}

fn display_repository(app: &Application, path: &str) -> String {
    if Path::new(path).starts_with(app.store.root().join("demos")) {
        "Attribution demo".into()
    } else if let Ok(imported) = app
        .store
        .get::<ImportedRepository>("imported_repository", path)
    {
        imported.repository_full_name
    } else {
        repository_name(path)
    }
}

pub fn scan_view(app: &Application, scan: &Scan) -> Value {
    let branch = scan
        .default_ref
        .clone()
        .unwrap_or_else(|| "Detached HEAD".into());
    let head = scan.refs.get(&branch).cloned().unwrap_or_default();
    let mut view = json!({"id":scan.id,"repository_path":scan.source_path,"repository_name":display_repository(app, &scan.source_path),"head":head,"branch":branch.strip_prefix("refs/heads/").unwrap_or(&branch),"total_commits":scan.commits.len(),"blockers":scan.blockers,"scanned_at":app.store.get::<String>("scan_time",&scan.id).unwrap_or_default(),"synthetic":Path::new(&scan.source_path).starts_with(app.store.root().join("demos")),"commits":scan.commits.iter().map(|c|json!({"oid":c.oid,"parents":c.parents,"tree_oid":c.tree_oid,"author_name":c.author_name,"author_email":c.author_email,"committer_name":c.committer_name,"committer_email":c.committer_email,"author_date":display_date(&c.author_date),"subject":c.message.lines().next().unwrap_or("(empty message)"),"blockers":c.unsupported_headers})).collect::<Vec<_>>()});
    if let Ok(imported) = app
        .store
        .get::<ImportedRepository>("imported_repository", &scan.source_path)
    {
        view["import_source"] = json!({"repository_full_name":imported.repository_full_name,"fetched_at":imported.fetched_at});
    }
    view
}

fn display_date(raw: &str) -> String {
    let Some((seconds, offset)) = raw.split_once(' ') else {
        return raw.into();
    };
    let (Ok(seconds), Ok(offset)) = (seconds.parse::<i64>(), offset.parse::<i32>()) else {
        return raw.into();
    };
    let offset_seconds = (offset / 100) * 3600 + (offset % 100) * 60;
    chrono::DateTime::from_timestamp(seconds, 0)
        .zip(chrono::FixedOffset::east_opt(offset_seconds))
        .map(|(date, offset)| date.with_timezone(&offset).to_rfc3339())
        .unwrap_or_else(|| raw.into())
}

pub fn plan_view(plan: &Plan) -> Value {
    let baseline = plan
        .scan
        .default_ref
        .as_ref()
        .and_then(|r| plan.scan.refs.get(r))
        .cloned()
        .unwrap_or_default();
    json!({"id":plan.id,"scan_id":plan.scan.id,"plan_hash":plan.content_hash,"selected_oids":plan.selected_oids,"affected_oids":plan.affected_oids,"baseline_oid":baseline,"blockers":plan.blockers,"edits":plan.selected_oids.iter().map(|oid|json!({"oid":oid,"field":"Author email","before":plan.selection.confirmed_email,"after":plan.selection.replacement_email})).collect::<Vec<_>>()})
}

pub fn operation_view(app: &Application, operation: &Operation) -> Result<Value> {
    let plan: Plan = app.store.get("plan", &operation.plan_id)?;
    let default_ref = plan.scan.default_ref.as_ref();
    let baseline = default_ref.and_then(|r| plan.scan.refs.get(r));
    let candidate = operation
        .prepared
        .as_ref()
        .and_then(|p| default_ref.and_then(|r| p.target_refs.get(r)));
    let checks = operation
        .prepared
        .as_ref()
        .map(|p| serde_json::to_value(&p.validation.checks))
        .transpose()?
        .unwrap_or(json!([]));
    let mut blockers = plan.blockers.clone();
    if let Some(error) = &operation.error {
        blockers.push(error.clone());
    }
    let local = matches!(
        operation.state.as_str(),
        "PLANNED"
            | "BACKUP_VERIFIED"
            | "PREPARING"
            | "VALIDATED"
            | "BLOCKED"
            | "FAILED_LOCAL"
            | "CANCELLED_BEFORE_PUSH"
    );
    let repository_status = if local {
        "Local operation only · remote unchanged"
    } else {
        "Remote outcome requires reconciliation · Inspector cannot resolve it"
    };
    Ok(
        json!({"id":operation.id,"plan_id":operation.plan_id,"repository_name":display_repository(app, &operation.repository_path),"state":operation.state,"created_at":operation.created_at,"selected_count":plan.selected_oids.len(),"rewritten_count":plan.affected_oids.len(),"baseline_oid":baseline,"candidate_oid":candidate,"backup_path":operation.prepared.as_ref().map(|p|&p.backup_path),"validation_checks":checks,"blockers":blockers,"repository_status":repository_status,"calendar_status":"Not observed by this Inspector"}),
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn dates_keep_original_offset() {
        assert_eq!(display_date("0 -0600"), "1969-12-31T18:00:00-06:00");
        assert_eq!(display_date("0 +0530"), "1970-01-01T05:30:00+05:30");
    }
}
