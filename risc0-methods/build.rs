fn main() {
    println!("cargo:rerun-if-env-changed=POWER_HOUSE_RISC0_DOCKER_BUILD");
    if std::env::var_os("POWER_HOUSE_RISC0_DOCKER_BUILD").is_some() {
        use risc0_build::{DockerOptionsBuilder, GuestOptionsBuilder};
        use std::collections::HashMap;

        let docker = DockerOptionsBuilder::default()
            .docker_container_tag(
                "r0.1.88.0@sha256:3e12f71bacd27527a61dea96fa0e53e468c99aa261d3a1019b593f6dbd943eb3",
            )
            .build()
            .expect("valid pinned RISC Zero Docker options");
        let guest = GuestOptionsBuilder::default()
            .use_docker(docker)
            .build()
            .expect("valid RISC Zero guest options");
        let mut options = HashMap::new();
        options.insert("power-house-sfcs-conformance-guest", guest);
        risc0_build::embed_methods_with_options(options);
    } else {
        risc0_build::embed_methods();
    }
}
