#!/home/shard/Documents/git-cloned/malbox/back-end/crates/malbox-plugin-sdk-python/.venv/bin/python3

"""Example Python host plugin that computes basic file information."""

import hashlib

import malbox_plugin_sdk as malbox


class HelloPlugin(malbox.Plugin):
    def on_start(self, config: dict[str, str]):
        malbox.info("HelloPlugin started")

    def on_task(self, ctx: malbox.Context):
        ctx.progress(0.0, "reading sample")

        data = ctx.task().sample_bytes()
        sha256 = hashlib.sha256(data).hexdigest()
        size = len(data)

        ctx.progress(0.5, "building report")

        ctx.results().push(malbox.PluginResult.json("file_info", {
            "sha256": sha256,
            "size": size,
        }))

        report = malbox.ReportBuilder("host-python-hello", "0.1.0")
        report.display_name("Python Hello")
        report.summary(f"SHA256: {sha256}, Size: {size} bytes")
        report.section("info", "File Information", [
            malbox.Block.kv([
                malbox.KvPair("SHA-256", sha256, mono=True),
                malbox.KvPair("Size", f"{size} bytes"),
            ])
        ])
        ctx.results().push(report.build())

        ctx.progress(1.0, "done")

    def on_stop(self):
        malbox.info("HelloPlugin stopping")

    def health_check(self) -> malbox.HealthStatus:
        return malbox.HealthStatus.Healthy()


malbox.run(HelloPlugin())
