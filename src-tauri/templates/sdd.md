# Software Design Document
<!-- section: Overview -->
<!-- directive: Write a technical overview of this design based on the session notes and tickets. Cover: (1) what is being built, (2) the key technical decisions and why they were made, and (3) what existing systems this integrates with or replaces. Assume the reader is a senior engineer or an AI coding agent that will implement from this spec. Be precise about technology choices and file locations. -->

<!-- section: Architecture -->
<!-- directive: Describe the high-level architecture. Identify the major components and how they interact. Describe data flow for the primary use cases. Call out architectural patterns used (event-driven, CQRS, etc.) and why they apply. If the session context mentions specific architectural constraints, reflect them accurately. Include a component diagram in text format if the architecture has more than 3 components. -->

<!-- section: File Map -->
<!-- directive: List every file that needs to be created or modified, with a one-line description of what each file is responsible for. Format as a table: | Action (Create/Modify) | File Path | Responsibility |. This is the implementation roadmap for the coding agent. Reference actual paths from the attached repos if available. -->

<!-- section: Data Model -->
<!-- directive: Describe the data model. List the key entities, their fields (with types), and relationships. If there are database schema changes, show the exact SQL or migration. Include indexing strategy and any denormalization decisions. If using an ORM, show the model definitions. Base this on the session context. -->

<!-- section: Interfaces -->
<!-- directive: Document the interfaces between components — API endpoints, function signatures, event contracts, or CLI commands. For each interface: name, inputs (with types), outputs (with types), error cases, and auth requirements. Write these precisely enough that an agent can implement both sides of each interface independently. -->

<!-- section: Error Handling -->
<!-- directive: Describe the error handling strategy. Cover: how errors propagate through layers, what errors are recoverable vs. fatal, what the user sees for each error type, and how errors are logged. List specific error scenarios from the session notes. For each, specify: the trigger, the expected behavior, and whether recovery is automatic or requires human intervention. -->

<!-- section: Implementation Notes -->
<!-- directive: List any implementation details that would trip up an AI coding agent: gotchas in the existing codebase, non-obvious constraints, libraries with quirky APIs, patterns that must be followed for consistency, environment-specific behavior. These are the things you'd tell a new team member on their first day working on this code. Reference @file mentions from the session if available. -->
