# qrab

Full instructions: ~/workspace/llm-rag/instructions/qrab.md
Knowledge docs: ~/workspace/llm-rag/docs/
Audit reports: ~/workspace/llm-rag/audits/
Plans: ~/workspace/llm-rag/plans/
Issues: ~/workspace/llm-rag/issues/

## Quick Commands

```sh
cargo test -p <crate>
cargo run -p qapi-codegen -- vendor/qemu --output-dir projects/qapi/src/generated
cargo run -p qapi-codegen -- vendor/qemu --qga --output-dir projects/qapi/src/qga
cargo clippy --workspace
```
