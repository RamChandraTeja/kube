#[macro_use] extern crate log;
use k8s_openapi::api::core::v1::Pod;
use serde_json::json;

use kube::{
    api::{Api, DeleteParams, ListParams, PatchParams, PostParams},
    client::APIClient,
    config,
};

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    std::env::set_var("RUST_LOG", "info,kube=debug");
    env_logger::init();
    let config = config::load_kube_config().await?;
    let client = APIClient::new(config);
    let namespace = std::env::var("NAMESPACE").unwrap_or("default".into());

    // Manage pods
    let pods: Api<Pod> = Api::namespaced(client, &namespace);

    // Create Pod blog
    info!("Creating Pod instance blog");
    let p = json!({
        "apiVersion": "v1",
        "kind": "Pod",
        "metadata": { "name": "blog" },
        "spec": {
            "containers": [{
              "name": "blog",
              "image": "clux/blog:0.1.0"
            }],
        }
    });

    let pp = PostParams::default();
    match pods.create(&pp, serde_json::to_vec(&p)?).await {
        Ok(o) => {
            let name = o.metadata.unwrap().name.unwrap();
            assert_eq!(p["metadata"]["name"], name);
            info!("Created {}", name);
            // wait for it..
            std::thread::sleep(std::time::Duration::from_millis(5_000));
        }
        Err(kube::Error::Api(ae)) => assert_eq!(ae.code, 409), // if you skipped delete, for instance
        Err(e) => return Err(e.into()),                        // any other case is probably bad
    }

    // Verify we can get it
    info!("Get Pod blog");
    let p1cpy = pods.get("blog").await?;
    let p1cpyspec = p1cpy.spec.unwrap();
    info!("Got blog pod with containers: {:?}", p1cpyspec.containers);
    assert_eq!(p1cpyspec.containers[0].name, "blog");

    // Replace its spec
    info!("Patch Pod blog");
    let patch = json!({
        "metadata": {
            "resourceVersion": p1cpy.metadata.unwrap().resource_version,
        },
        "spec": {
            "activeDeadlineSeconds": 5
        }
    });
    let patch_params = PatchParams::default();
    let p_patched = pods
        .patch("blog", &patch_params, serde_json::to_vec(&patch)?)
        .await?;
    assert_eq!(p_patched.spec.unwrap().active_deadline_seconds, Some(5));

    let lp = ListParams::default().fields(&format!("metadata.name={}", "blog")); // only want results for our pod
    for p in pods.list(&lp).await? {
        info!("Found Pod: {}", p.metadata.unwrap().name.unwrap());
    }

    // Delete it
    let dp = DeleteParams::default();
    pods.delete("blog", &dp).await?.map_left(|pdel| {
        assert_eq!(pdel.metadata.unwrap().name.unwrap(), "blog");
        info!("Deleting blog pod started");
    });

    Ok(())
}
