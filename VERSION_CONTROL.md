# Semantic version control and Git

Vibe uses Git, but does not ask Git to understand program internals. Git remains
the distributed repository, authentication, signing, remote, fork, worktree, CI,
fetch, pull, and push layer. `vibec` supplies immutable semantic history, bounded
diffs, and three-way merging for Vibe program objects.

## Implemented model

A `.vibepack` contains:

```text
object revision = SHA-256(canonical typed object)
commit tree     = semantic identity -> object revision
commit revision = SHA-256(parents, basis, name, commit tree)
branch          = name -> commit revision
```

Objects and commits are immutable and content-addressed. Commits form a DAG and
retain all historical object versions. A branch is only a movable head. Builds,
queries, and validation can select a branch or exact revision. A transaction names
its target branch, base commit, read-set object revisions, and expected revisions
for writes. The environment validates and atomically advances the branch.

Stale transactions may automatically rebase when every declared read and write
precondition still holds. This permits many agents to commit disjoint objects from
the same base. Two different replacements of one object conflict. Transactions
that call an unchanged function must declare that function's revision in their
read set, preventing an agent from silently relying on stale interfaces.

Semantic three-way merge compares identity-to-object-revision trees:

```text
target == source       -> retain either
target == merge base   -> take source
source == merge base   -> take target
otherwise              -> conflict on that semantic identity
```

A successful non-fast-forward merge records both parents and is type-, unit-, and
effect-checked before the target branch moves. A conflict reports bounded records
containing identity and base/target/source object revisions and changes nothing.

## Git mapping

| Operation | Git responsibility | Vibe responsibility |
|---|---|---|
| Edit | Preserve checkout | Typed semantic transaction |
| Commit | Signed repository checkpoint | Immutable object and commit revisions |
| Branch | Distributed collaboration/worktree boundary | Optional in-pack semantic branch |
| Fork | Repository ownership and remote | Pack history remains portable |
| Diff | Select Git revisions | Render bounded semantic object changes |
| Merge | Invoke the configured driver | Three-way identity/object merge and validation |
| Fetch | Transfer Git objects without mutation | No program mutation |
| Pull | Fetch and Git merge | Validate and semantically merge `.vibepack` |
| Push | Authentication, policy, transfer | CI validates semantic roots and evidence |

Run `vibec env git-configure PROGRAM.vibepack` once per clone. It installs local
`merge.vibe` and `diff.vibe` drivers. The tracked `.gitattributes` assigns both to
`.vibepack` files. Git then invokes `vibec` automatically. The merge driver never
executes the program; it verifies hashes, merges semantic trees, reconstructs the
selected program, and runs static validation. The text-conversion driver emits one
compact line per semantic object with its revision, interface, calls, and effects;
it never emits implementation serialization.

The recommended shared workflow is:

1. `git fetch` and inspect the semantic diff against the remote head.
2. Create a Git branch/worktree for each agent task. Use in-pack branches only for
   cheap local alternatives within that task.
3. Submit Vibe transactions. Several related operations form one atomic semantic
   commit; no source files are patched.
4. Run affected validation, then Git-commit the `.vibepack` checkpoint and evidence
   manifests. A Git commit may bundle several small semantic commits.
5. Integrate shared work with Git merge or `git pull --no-rebase`; the Vibe driver
   retains both semantic parents. Avoid rebasing already-shared semantic history.
6. Push normally. Server CI rejects invalid pack hashes, unresolved conflicts,
   missing required evidence, or forbidden capability/effect expansion.

Git branches are the authoritative distributed branches. In-pack branches are
useful for local experiments and agent proposals, but should be merged or removed
before publishing unless the project deliberately exposes them. This avoids two
competing long-lived branch namespaces.

## Scaling beyond the bootstrap bundle

The current `.vibepack` is a self-contained bundle and rewrites the bundle on each
commit. That is correct and convenient for small programs, interchange, releases,
and tests, but is not the final large-repository storage layout.

The production checkout will store a bounded number of append-only compressed
pack segments under `.vibe/packs/`, plus a small ref/root manifest. It will not
create one JSON or one filesystem file per semantic object. New commits append
objects not already present; periodic transactional compaction combines small
segments. Git efficiently transfers immutable packs, while a gitignored SQLite
index provides local dependency, reachability, and object-location queries.
Portable `.vibepack` export bundles the selected reachable history when a single
file is desired.

Compilation caches, tensor data, checkpoints, HDF5 products, and generated
artifacts are not semantic version-control objects. They are addressed by hashes
and stored in the artifact cache, Git LFS, or an external artifact service as
project policy requires. Semantic commits retain their typed metadata, hashes,
provenance, and reproducibility requirements.

## LLM interface rules

- Agents query revisions and semantic neighborhoods, never decode CBOR or Git
  blobs directly.
- Diffs default to changed identities, interfaces, effects, dependencies,
  validation impact, and evidence; bodies are expanded only under a context budget.
- Every mutation carries read/write preconditions, making retries idempotent and
  stale context detectable.
- Merge conflicts are semantic objects with valid resolution operations, not text
  markers or line ranges.
- Pull requests summarize semantic and evidence changes; raw binary size is not a
  review signal.
- Remote packs are untrusted input: hashes, schemas, size limits, references,
  effects, and static validity are checked before a branch moves.

## Remaining production work

The bootstrap implements immutable revisions, history, branches, historical
queries/builds, object-level three-way merge, Git merge integration, and semantic
Git diffs. It does not yet implement persistent merge-resolution sessions,
branch deletion, reflogs, signed semantic commits, pack segmentation/compaction,
remote partial-pack negotiation, garbage collection, or server-side receive
hooks. These are required before claiming production-scale version control.
