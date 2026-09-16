# Isolated publication candidate

`prepare_publication.py` validates the selected local branch and prints a dry-run
plan by default. It derives the source identity from the oldest post-baseline
commit without printing its email, accepts that identity plus the approved
noreply identity with the same name, rejects signed commits and any third
identity, and reports the old-to-new commit map. The current Git configuration
does not determine which historical identity is redacted.

The explicit execution mode creates a new bare repository below
`.local/publication/`. It imports the exact approved baseline ancestry and only
the trees/blobs needed by the selected branch, then writes new post-baseline
commit objects with the approved noreply email. It imports no tags, remotes,
other branches, superseded sensitive-email commit objects, working-tree changes,
or index changes. An already-noreply commit keeps its original object ID when its
parent is also unchanged. The tool performs no network operation and never changes
the source.

From the repository root:

```powershell
python scripts/publication/prepare_publication.py `
  --noreply-email '<approved-noreply-address>'
python -m unittest scripts.publication.test_prepare_publication
```

After reviewing the dry-run map, create a new candidate at the default ignored
location:

```powershell
python scripts/publication/prepare_publication.py `
  --noreply-email '<approved-noreply-address>' --execute
```

Use `--destination .local/publication/<new-name>.git` to select another new path
within the guarded publication directory. `--gc` is optional and only operates
on a newly created, verified candidate. Inspect `PUBLICATION_REPORT.txt` and
`PUBLICATION_SHA_MAP.tsv` inside the candidate before any separately authorized
publication action. The tool does not configure a remote or push anything.
It does not read or modify `.local/maintainer-notes/` or any other private
archive; execution writes only the newly claimed candidate directory.
