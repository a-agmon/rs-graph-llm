use std::sync::Arc;

use async_trait::async_trait;
use graph_flow::{Context, FlowRunner, GraphBuilder, InMemorySessionStorage, NextAction, Session, SessionStorage, Task, TaskResult, FanOutTask};

struct Prepare;
struct ChildA;
struct ChildB;
struct Consume;

#[async_trait]
impl Task for Prepare {
    fn id(&self) -> &str { "prepare" }
    async fn run(&self, ctx: Context) -> graph_flow::Result<TaskResult> {
        ctx.set("input", "hello".to_string()).await;
        Ok(TaskResult::new(Some("prepared".to_string()), NextAction::Continue))
    }
}

#[async_trait]
impl Task for ChildA {
    fn id(&self) -> &str { "child_a" }
    async fn run(&self, ctx: Context) -> graph_flow::Result<TaskResult> {
        let inp: String = ctx.get("input").await.unwrap_or_default();
        ctx.set("a_out", format!("{}-A", inp)).await;
        Ok(TaskResult::new(Some("A done".to_string()), NextAction::End))
    }
}

#[async_trait]
impl Task for ChildB {
    fn id(&self) -> &str { "child_b" }
    async fn run(&self, ctx: Context) -> graph_flow::Result<TaskResult> {
        let inp: String = ctx.get("input").await.unwrap_or_default();
        ctx.set("b_out", format!("{}-B", inp)).await;
        Ok(TaskResult::new(Some("B done".to_string()), NextAction::End))
    }
}

#[async_trait]
impl Task for Consume {
    fn id(&self) -> &str { "consume" }
    async fn run(&self, ctx: Context) -> graph_flow::Result<TaskResult> {
        // Read aggregated responses stored by the fanout task
        let a_resp: Option<String> = ctx.get("fanout.child_a.response").await;
        let b_resp: Option<String> = ctx.get("fanout.child_b.response").await;
        let final_msg = format!("A={:?}, B={:?}", a_resp, b_resp);
        Ok(TaskResult::new(Some(final_msg), NextAction::End))
    }
}

#[tokio::main]
async fn main() -> graph_flow::Result<()> {
    let prepare: Arc<dyn Task> = Arc::new(Prepare);
    let child_a: Arc<dyn Task> = Arc::new(ChildA);
    let child_b: Arc<dyn Task> = Arc::new(ChildB);
    let fanout = FanOutTask::new("fan", vec![child_a.clone(), child_b.clone()]);
    let consume: Arc<dyn Task> = Arc::new(Consume);

    let graph = Arc::new(
        GraphBuilder::new("fanout_demo")
            .add_task(prepare.clone())
            .add_task(fanout.clone())
            .add_task(consume.clone())
            .add_edge(prepare.id(), fanout.id())
            .add_edge(fanout.id(), consume.id())
            .build(),
    );

    // Session and runner
    let storage = Arc::new(InMemorySessionStorage::new());
    let runner = FlowRunner::new(graph.clone(), storage.clone());

    let session = Session::new_from_task("demo_session".to_string(), prepare.id());
    storage.save(session).await?;

    // Step 1: prepare -> pause to fan
    let r1 = runner.run("demo_session").await?;
    println!("step1: {:?}", r1.status);

    // Step 2: execute fanout (runs children concurrently)
    let r2 = runner.run("demo_session").await?;
    println!("step2: {:?}", r2.status);

    // Step 3: consume
    let r3 = runner.run("demo_session").await?;
    println!("step3: {:?}, resp={:?}", r3.status, r3.response);

    Ok(())
}

