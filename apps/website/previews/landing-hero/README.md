# Landing-page hero Preview cycle

This directory is the public, reviewable record for the first Chaft Previews
cycle. Four branches start from one exact protected `main` revision, build four
independent landing-page hero directions, and publish immutable static
artifacts to four fixed Preview slots.

Production remains unchanged until the Preview cycle selects one version and a
fresh promotion pull request passes the existing production checks.

## Fixed Preview slots

| Preview | Branch | Worker | Domain | GitHub environment |
| --- | --- | --- | --- | --- |
| Hero 1 | `preview/landing-hero-1` | `chaft-website-hero-1` | `hero-1.chaft.ai` | `chaft-preview-hero-1` |
| Hero 2 | `preview/landing-hero-2` | `chaft-website-hero-2` | `hero-2.chaft.ai` | `chaft-preview-hero-2` |
| Hero 3 | `preview/landing-hero-3` | `chaft-website-hero-3` | `hero-3.chaft.ai` | `chaft-preview-hero-3` |
| Hero 4 | `preview/landing-hero-4` | `chaft-website-hero-4` | `hero-4.chaft.ai` | `chaft-preview-hero-4` |

The mapping is closed and exact. Branch names, Worker names, domains, and
GitHub environments cannot be supplied by pull-request code or workflow input.
`workers.dev`, Cloudflare-generated preview URLs, and wildcard domains remain
disabled.

## Starting a Preview cycle

1. Merge and provision the shared Chaft Previews foundation.
2. Record the exact protected `main` commit in `baseRevision`.
3. Record an immutable Figma version and one frame node for each Preview.
4. Create all four branches and worktrees from `baseRevision`.
5. Set the cycle to `active` and each slot to `ready`, then open four draft pull requests.
6. Allow the credential-free candidate workflow to build and test each branch.
7. Let the trusted default-branch workflow verify and deploy the immutable
   artifact to its exact Preview slot.

Every Preview candidate uses:

```text
CHAFT_DEPLOYMENT_MODE=preview
CHAFT_PREVIEW_BRANCH=preview/landing-hero-N
SITE_URL=https://hero-N.chaft.ai
```

The branch and URL must use the same slot number. Any other value fails closed.

## Invariant product contract

The four versions may change only the hero's visual treatment and the layout
needed to support it. They share:

- the exact headline and the body-copy digest in `preview-cycle.json`;
- the three call-to-action labels and destinations;
- the canary warning and all security claims;
- the header, navigation, footer, and content following the hero;
- Chillax for body copy only; and
- Space Grotesk for headings, navigation, buttons, labels, and other UI text.

Changing an invariant requires a separate reviewed foundation change before a
new Preview cycle starts. A version branch must not silently widen its scope.

## Common QA

Install the pinned dependencies and Playwright browsers once:

```sh
corepack pnpm install --frozen-lockfile
corepack pnpm exec playwright install --with-deps chromium firefox webkit
```

Then run the complete Preview gate from `apps/website`:

```sh
CHAFT_DEPLOYMENT_MODE=preview \
CHAFT_PREVIEW_BRANCH=preview/landing-hero-1 \
SITE_URL=https://hero-1.chaft.ai \
corepack pnpm qa
```

The gate builds the static site once, then runs:

- Chromium checks at 320, 390, 768, and 1440 CSS pixels;
- Firefox and WebKit checks at 1440 CSS pixels;
- invariant copy, action, typography, overflow, and layout assertions;
- WCAG A/AA automated checks with serious and critical findings blocked;
- deterministic reduced-motion screenshots attached to the test report; and
- Lighthouse performance, accessibility, and best-practice budgets.

Focused commands are available as `pnpm qa:accessibility`,
`pnpm qa:visual`, and `pnpm qa:lighthouse`. Automated checks complement rather
than replace keyboard, touch, reduced-motion, VoiceOver, and lower-powered
device review.

## Review and selection

Use neutral Preview labels during review. Give each participant the same task,
rotate presentation order, and record comprehension, task success, confidence,
trust, performance, and accessibility observations. Preference is supporting
evidence rather than the only selection criterion.

Record the selected slot and decision record in `preview-cycle.json`. Do not
merge all four Preview pull requests. Create a fresh descriptive branch from
the latest `main`, replay only the selected production changes, and use the
existing protected production workflow.

## Reset and rollback

Preview Workers and domains are reusable infrastructure. Closing or deleting a
branch does not remove deployed content.

- Reset a completed slot by deploying the reviewed inactive build from
  protected `main`.
- Roll back only to an explicitly named retained Worker version whose source
  commit and deployment record match.
- Never infer “previous,” delete a domain automatically, or modify the
  production Worker as part of Preview cleanup.

The reviewed private governance record is
`cloudflare/website-previews/component.yaml`. It defines credential scope,
retention, access mode, verification, reset, and rollback policy without
placing private operational data in this public repository.
