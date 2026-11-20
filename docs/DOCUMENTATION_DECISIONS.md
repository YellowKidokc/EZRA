# Documentation revision rationale — November 20, 2026

This records the review resolutions in README v0.2. It does not claim the implementation or acceptance tests have been completed.

| Issue | Resolution |
|---|---|
| Automatic versus optional ID writes | Both documents now say off by default; explicit opt-in, format-aware metadata and immediate post-write hashes/history |
| Edited but unmoved files | Same logical identity; append a content revision, preserve old hash/history |
| Unrelated replacement at same path | Explicit conflicting evidence creates a hold; ordinary atomic saves are not automatically replacements |
| Old proof receipt after an edit | Remains attached to its original revision/hash; not inherited by the edited content |
| Partial/unreachable scans | Missing applies only inside successfully enumerated coverage; unavailable and excluded areas remain unknown |
| Database mismatch | Five v0.1 tables: records, labels, links, locations and aliases |
| “Revisions table” versus five tables | Store append-only revision observations in locations for v0.1. A dedicated revisions table would be a later sixth-table migration |
| Recipes | Deferred, including their storage/execution grammar |
| Repository identity | EZRA is the standalone implementation home; Schema_Engine is a reference/possible adapter source |
| Runtime libraries | Declare PyYAML; built-in UUIDv7 on Python 3.14+, named uuid6 backport on older supported Python, with serialized generation/conformance tests |
| Folder identity | Same-path continuity; automatic folder relocation is explicitly not promised in v0.1 |
| Quick hashes | Candidates only; full hashes required for confirmed content identity |
| Mapped network database paths | Refuse mapped/redirected network locations as well as literal UNC |
| URI normalization | Trailing slash, encoded aliases, case/Unicode and empty-valued flags covered explicitly |
| Word/browser behavior | Manual integration and security prompts separated from automated parser/registration checks |
| NAS-only references | Portable reference summaries included; private machine configuration remains local |
| MIM counts | Eleven loose root candidates and six numbered-suffix variants, not every paper twice/seven variants |
| Path-driven classification | Path scope is evidence of physical containment, not automatic semantic ownership |
| AXIOM source families | Preserve multiple legitimate series and unique archives; keeper selection remains explicit |

The prior README remains in Git history. Reference summaries are attributed adaptations rather than verbatim substitutes for the full internal source documents.

## Source checks for dependency choices

- [Python UUID documentation](https://docs.python.org/3.14/library/uuid.html).
- [PyYAML package metadata](https://pypi.org/project/PyYAML/).
- [uuid6 package and API documentation](https://pypi.org/project/uuid6/).

PyYAML 6.0.3 and uuid6 2025.0.1 were available during the documentation update. This establishes availability, not successful install/runtime/matrix acceptance. The compatibility generator is documented as not thread-safe; the build must serialize generation and test concurrent callers.
