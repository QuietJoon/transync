# transync — full-coverage smoke fixture

This document exercises every sync-relevant block kind at least once, plus a thematic break and a block-level image. It is the canonical input for SCN-12 (CLI end-to-end), SCN-13 (manual demo smoke), and SCN-14 (full-document reparse).

## Section A — prose, code, and lists

The intent of section A is to combine plain prose with a small code block and a flat list, so the regenerated MD reparses to the same kind sequence.

```rust
pub fn smoke() -> &'static str {
    "transync"
}
```

- entry one
- entry two
- entry three

---

## Section B — table, blockquote, and image

The intent of section B is to combine a structurally-constrained block (the table) with a container block (the blockquote) and a block-level image.

| field | kind     | notes                                |
|-------|----------|--------------------------------------|
| id    | u64      | unique                                |
| name  | string   | display                               |
| flag  | bool     | feature toggle                        |

> The renderer attaches `data-sync-id` to every block with a sync role.
> The JS engine reads only those attributes; pane layout is irrelevant.

![transync logo placeholder](./logo.png)

<details>
<summary>Bundled extras</summary>

Extra bundled paragraph.

</details>

<div align="center">
<b>Hero banner</b>
</div>
