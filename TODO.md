- [x] Add roles as config
- [ ] Add outside type files as env vars
- [ ] Directives
- [ ] Tests
- [ ] Documentation of config
- [ ] More comments + modularisation where possible
- [ ] Cache hash of all migrations + the branch name and only run full refresh when both change between runs. This would likely mean that a different branch with some extra migrations has been checked out and a refresh is required. But generally if working on the same branch the current db would be used instead so no migration chain.