# GergiOS

## Overview

This is a modernized version of Minix, a microkernel-based operating system. This project aims to modernize the Minix codebase through comprehensive refactoring, architecture updates, and technology migration.

## License Change

This project has been relicensed from the original BSD-style license to **GPLv2 or later**.

### Historical License

The original Minix code was licensed under a BSD-style license by Vrije Universiteit, Amsterdam:

```
Copyright (c) 1987, 1997, 2006, Vrije Universiteit, Amsterdam, The Netherlands
```

### Reason for Relicensing

Due to the extensive modernization and rewriting of the codebase as outlined in `TODO.md` and the `planning/` directory, the majority of the code has been rewritten or significantly modified. As a result, the project has been relicensed to GPLv2 or later to:

- Better align with modern open source practices
- Facilitate collaboration with other GPL-licensed projects
- Provide stronger copyleft protection for the modernized codebase
- Enable integration with modern open source ecosystems

### License Status

- **Rewritten and newly added code**: Licensed under GPLv2 or later
- **Remaining original code from Vrije Universiteit**: Continues to be available under the original BSD-style license terms

The relicensing to GPLv2 or later applies only to the rewritten and newly added code. Any remaining original code from Vrije Universiteit that has not been rewritten continues to be available under the original BSD-style license terms.

For more information about the modernization project and code changes, see the `TODO.md` file and the `planning/` directory.

## Modernization Goals

The modernization project aims to:

- Update architecture support (x86_64, ARM64)
- Migrate from OpenSSL 0.9.8 to wolfSSL
- Modernize the build system
- Improve security features
- Enhance performance
- Update documentation
- Improve testing infrastructure

## Documentation

- **TODO.md**: Overall modernization roadmap
- **planning/**: Detailed planning documents for each modernization area
- **docs/**: Technical documentation

## Status

This is an active modernization project. See `TODO.md` for current progress and next steps.

## Contributing

Contributions are welcome. Please note that contributions to this project will be licensed under GPLv2 or later to maintain license consistency.

## Contact

For questions about the modernization project or licensing, please refer to the planning documents in the `planning/` directory.
