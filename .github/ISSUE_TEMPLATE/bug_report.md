---
name: Bug report
about: Create a report to help us improve
labels: bug
title: "[Bug]: "
assignees: ""
body:
  - type: textarea
    id: what-happened
    attributes:
      label: What happened?
      description: Describe the bug and expected behavior
      placeholder: Clear description...
    validations:
      required: true
  - type: textarea
    id: steps
    attributes:
      label: Steps to reproduce
      description: Minimal steps to reproduce the issue
      placeholder: 1) ..., 2) ...
  - type: textarea
    id: logs
    attributes:
      label: Logs/Artifacts
      description: Include relevant logs or artifacts (e.g., JSONL, lcov.info)
  - type: input
    id: version
    attributes:
      label: Version/Commit
      description: Output of `git rev-parse --short HEAD` or release tag
