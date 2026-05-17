# Test Plan
<!-- section: Overview -->
<!-- directive: Write a brief overview of this test plan. Describe what is being tested (the feature or system from the session context), the testing goals, and the scope. Mention the key risks this test plan mitigates. Note: in an agentic workflow, most test authoring and execution is handled by AI agents — this plan defines WHAT to test and the verification strategy, not manual QA steps. -->

<!-- section: Automated Test Strategy -->
<!-- directive: Describe the automated testing strategy — this is the primary verification method. Cover: unit tests (what modules, what coverage targets), integration tests (what boundaries to test), and end-to-end tests (what user flows). For each category, describe what an AI coding agent should generate: test file locations, testing frameworks to use, fixture/mock strategy. Reference the actual codebase structure from attached repos if available. Specify which tests run in CI and which run locally. -->

<!-- section: Agent-Generated Test Cases -->
<!-- directive: Write test case specifications that an AI coding agent can implement directly. For each test: a unique ID (TC-1, TC-2, ...), descriptive name, the module/file under test, input, expected output, and edge conditions. Cover the happy path for each major feature and at least 3 negative cases. Write these as specifications, not manual steps — an agent will convert them to actual test code. Base on actual tickets and requirements in the session. -->

<!-- section: Human Verification Points -->
<!-- directive: List the things that CANNOT be verified by automated tests and require human judgment. These include: UX quality, business logic correctness, visual design fidelity, performance perception, accessibility experience, and security architecture review. For each item, describe what specifically to check and what "good" looks like. Keep this list short — if it can be automated, it should be in the automated section. -->

<!-- section: Edge Cases and Failure Modes -->
<!-- directive: Identify edge cases and failure scenarios. For each: describe the scenario, expected behavior, and whether the test should be automated (most should). Include: boundary conditions, concurrent access, network failures, large inputs, empty states, permission edge cases. Draw from session notes and reason about what could go wrong. An AI agent will implement these as test cases. -->

<!-- section: Release Verification Checklist -->
<!-- directive: A final checklist for release readiness. Split into: **Automated Gates** (CI pipeline must pass: all tests green, type check clean, lint clean, security scan clean, performance benchmarks within tolerance) and **Human Sign-offs** (architecture review approved, UX walkthrough done, stakeholder demo completed). Each item is pass/fail. -->
