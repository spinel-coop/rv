# `rv` Settings

This document describes all configuration settings supported by `rv`, how each one is resolved, and how configuration precedence works.

Example `rv.kdl`:

```kdl
rv {
  install-path "/custom/gems"
  update-mode "warning"
}
```

## Configuration precedence

When the same setting is defined in multiple places, `rv` resolves it in the following order (highest to lowest priority):

1. **Environment variables** (e.g. `RV_INSTALL_PATH`)
2. **Project-level config** (`./rv.kdl`, `./.config/rv.kdl`, or `./.config/rv/rv.kdl`)
3. **Global user config** (`~/.rv.kdl`, `~/.config/rv.kdl`, or `~/.config/rv/rv.kdl`)

In other words: **ENV overrides local, local overrides global.**

---

## `install-path`

**Description:** Custom path where `rv` installs gems.

**Default:** Resolved in the following order:

1. Bundler's configured `path` (e.g. from `.bundle/config` or `BUNDLE_PATH`).
2. The default `GEM_HOME` of the currently active Ruby version.

**Allowed values:** Any valid filesystem path.

**Example:**

```kdl
rv {
  install-path "/custom/gems"
}
```

**Environment variable override:** `RV_INSTALL_PATH`

---

## `update-mode`

**Description:** Controls how `rv` handles updating itself.

**Default:** `"install"`

**Allowed values:**

| Value | Behaviour |
| --------- | ----------------------------------------------------------------- |
| `"none"` | `rv` never updates itself automatically. Updates are ignored. |
| `"warning"` | `rv` notifies you when a new version is available but does not install it. |
| `"install"` | `rv` automatically downloads and installs updates when available. |

**Example:**

```kdl
rv {
  update-mode "warning"
}
```

**Environment variable override:** `RV_UPDATE_MODE`
