use crate::db::Db;
use chrono::{Datelike, Local, NaiveDate, TimeZone, Timelike};
use serde::Serialize;
use std::collections::BTreeMap;

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Bucket {
    /// `2026-10-28` for days, `2026-W44` for weeks.
    pub key: String,
    pub label: String,
    pub sub_label: String,
    pub start_ms: i64,
    pub end_ms: i64,
    pub focus_seconds: i64,
    pub pomodoros: i64,
    pub completed: i64,
    pub aborted: i64,
    pub interruptions: i64,
    pub interrupted_seconds: i64,
    pub break_seconds: i64,
    pub tasks: usize,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Summary {
    pub focus_seconds: i64,
    pub pomodoros: i64,
    pub completed: i64,
    pub aborted: i64,
    pub interruptions: i64,
    pub interrupted_seconds: i64,
    pub break_seconds: i64,
    pub avg_focus_seconds: i64,
    pub best_bucket_label: String,
    pub best_focus_seconds: i64,
    pub distinct_tasks: usize,
    /// Completed work sessions / (completed + aborted).
    pub completion_rate: f64,
    /// Average focus minutes per started pomodoro.
    pub avg_session_minutes: f64,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TaskAgg {
    pub task_id: Option<i64>,
    pub title: String,
    pub pomodoros: i64,
    pub focus_seconds: i64,
    pub interruptions: i64,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct StatsReport {
    pub granularity: String,
    pub buckets: Vec<Bucket>,
    pub summary: Summary,
    pub tasks: Vec<TaskAgg>,
    /// Work-session starts per hour of day, index 0..23.
    pub hour_heat: Vec<i64>,
    pub range_start_ms: i64,
    pub range_end_ms: i64,
}

fn start_of_day_ms(d: NaiveDate) -> i64 {
    let naive = d.and_hms_opt(0, 0, 0).expect("valid midnight");
    Local
        .from_local_datetime(&naive)
        .single()
        .map(|dt: chrono::DateTime<Local>| dt.timestamp_millis())
        .unwrap_or_else(|| naive.and_utc().timestamp_millis())
}

fn monday_of(d: NaiveDate) -> NaiveDate {
    let offset = d.weekday().num_days_from_monday() as i64;
    d - chrono::Duration::days(offset)
}

/// `count` consecutive buckets ending with the current one.
fn build_window(granularity: &str, count: i64) -> Vec<(String, String, String, i64, i64)> {
    let today = Local::now().date_naive();
    let count = count.clamp(1, 180);
    let mut out = Vec::with_capacity(count as usize);
    for i in (0..count).rev() {
        if granularity == "week" {
            let start = monday_of(today) - chrono::Duration::weeks(i);
            let end = start + chrono::Duration::days(7);
            let iso = start.iso_week();
            out.push((
                format!("{}-W{:02}", iso.year(), iso.week()),
                format!("第{}周", iso.week()),
                format!(
                    "{} ~ {}",
                    start.format("%m-%d"),
                    (end - chrono::Duration::days(1)).format("%m-%d")
                ),
                start_of_day_ms(start),
                start_of_day_ms(end),
            ));
        } else {
            let day = today - chrono::Duration::days(i);
            let next = day + chrono::Duration::days(1);
            let label = if i == 0 { "今天".to_string() } else { format!("{}", day.format("%m-%d")) };
            out.push((
                day.format("%Y-%m-%d").to_string(),
                label,
                day.format("%m-%d").to_string(),
                start_of_day_ms(day),
                start_of_day_ms(next),
            ));
        }
    }
    out
}

fn new_bucket(key: String, label: String, sub_label: String, start_ms: i64, end_ms: i64) -> Bucket {
    Bucket {
        key,
        label,
        sub_label,
        start_ms,
        end_ms,
        focus_seconds: 0,
        pomodoros: 0,
        completed: 0,
        aborted: 0,
        interruptions: 0,
        interrupted_seconds: 0,
        break_seconds: 0,
        tasks: 0,
    }
}

pub fn query(db: &Db, granularity: &str, count: i64) -> StatsReport {
    let window = build_window(granularity, count);
    let range_start = window.first().map(|w| w.3).unwrap_or(0);
    let range_end = window.last().map(|w| w.4).unwrap_or(0);
    let mut buckets: Vec<Bucket> = window
        .iter()
        .map(|(k, l, s, st, en)| new_bucket(k.clone(), l.clone(), s.clone(), *st, *en))
        .collect();

    let sessions = db.sessions_in(range_start, range_end);
    let mut task_titles: BTreeMap<i64, TaskAgg> = BTreeMap::new();
    let mut untitled: BTreeMap<String, TaskAgg> = BTreeMap::new();
    let mut hour_heat = vec![0i64; 24];
    let mut task_sets: Vec<BTreeMap<String, ()>> = vec![BTreeMap::new(); buckets.len()];

    for s in &sessions {
        let Some(idx) = buckets.iter().position(|b| s.started_at >= b.start_ms && s.started_at < b.end_ms) else {
            continue;
        };
        let b = &mut buckets[idx];
        let work = s.kind == "work";
        b.interruptions += s.interruptions;
        if work {
            b.focus_seconds += s.focus_seconds;
            b.interrupted_seconds += s.paused_seconds;
            if s.status == "completed" {
                b.completed += 1;
                b.pomodoros += 1;
            } else {
                b.aborted += 1;
            }
            let hour = chrono::DateTime::from_timestamp_millis(s.started_at)
                .map(|utc| utc.with_timezone(&Local).hour() as usize)
                .unwrap_or(0);
            hour_heat[hour.min(23)] += 1;
        } else {
            b.break_seconds += s.focus_seconds;
        }

        // Task attribution: bound tasks use the live title, ad-hoc descriptions group by text.
        let entry = match s.task_id {
            Some(id) => {
                let agg = task_titles.entry(id).or_insert_with(|| TaskAgg {
                    task_id: Some(id),
                    title: db.task_get(id).map(|t| t.title).unwrap_or_else(|| "已删除任务".into()),
                    pomodoros: 0,
                    focus_seconds: 0,
                    interruptions: 0,
                });
                if agg.title == "已删除任务" && !s.description.is_empty() {
                    agg.title = s.description.clone();
                }
                agg
            }
            None => {
                let key = if s.description.is_empty() { "未命名专注".to_string() } else { s.description.clone() };
                let agg = untitled.entry(key.clone()).or_insert_with(|| TaskAgg {
                    task_id: None,
                    title: key,
                    pomodoros: 0,
                    focus_seconds: 0,
                    interruptions: 0,
                });
                agg
            }
        };
        if work {
            entry.focus_seconds += s.focus_seconds;
            entry.interruptions += s.interruptions;
            if s.status == "completed" {
                entry.pomodoros += 1;
            }
        }
        let name = match s.task_id {
            Some(id) => format!("task:{id}"),
            None => format!("desc:{}", s.description),
        };
        task_sets[idx].insert(name, ());
    }

    let mut tasks: Vec<TaskAgg> = task_titles.into_values().chain(untitled.into_values()).collect();
    tasks.retain(|t| t.focus_seconds > 0 || t.pomodoros > 0);
    tasks.sort_by(|a, b| b.focus_seconds.cmp(&a.focus_seconds).then(b.pomodoros.cmp(&a.pomodoros)));
    tasks.truncate(40);

    for (i, b) in buckets.iter_mut().enumerate() {
        b.tasks = task_sets[i].len();
    }

    let focus: i64 = buckets.iter().map(|b| b.focus_seconds).sum();
    let completed: i64 = buckets.iter().map(|b| b.completed).sum();
    let aborted: i64 = buckets.iter().map(|b| b.aborted).sum();
    let best = buckets.iter().max_by_key(|b| b.focus_seconds).cloned();
    let non_empty = buckets.iter().filter(|b| b.focus_seconds > 0).count() as i64;
    let distinct_tasks: usize = tasks.len();

    let summary = Summary {
        focus_seconds: focus,
        pomodoros: completed,
        completed,
        aborted,
        interruptions: buckets.iter().map(|b| b.interruptions).sum(),
        interrupted_seconds: buckets.iter().map(|b| b.interrupted_seconds).sum(),
        break_seconds: buckets.iter().map(|b| b.break_seconds).sum(),
        avg_focus_seconds: if non_empty > 0 { focus / non_empty } else { 0 },
        best_bucket_label: best.as_ref().map(|b| b.label.clone()).unwrap_or_default(),
        best_focus_seconds: best.map(|b| b.focus_seconds).unwrap_or(0),
        distinct_tasks,
        completion_rate: if completed + aborted > 0 {
            completed as f64 / (completed + aborted) as f64
        } else {
            0.0
        },
        avg_session_minutes: if completed + aborted > 0 {
            focus as f64 / (completed + aborted) as f64 / 60.0
        } else {
            0.0
        },
    };

    StatsReport {
        granularity: granularity.to_string(),
        buckets,
        summary,
        tasks,
        hour_heat,
        range_start_ms: range_start,
        range_end_ms: range_end,
    }
}
