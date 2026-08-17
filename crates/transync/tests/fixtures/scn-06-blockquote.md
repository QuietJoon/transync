## On block-level synchronization

> Sync-by-percentage breaks the moment one pane renders a code block taller than its source counterpart. Anchoring on stable block IDs is the only approach that survives translation-induced length changes.
>
> The same is true for tables: a 50-row source table whose translated cells wrap to multiple lines will desynchronize a percentage-based scroller almost immediately.
>
> - Block IDs travel through the pipeline unchanged.
> - The renderer emits them as `data-sync-id`.
> - The JS engine reads only those attributes; layout is irrelevant.

A blockquote with two nested paragraphs and one nested list, as required by the SCN-06 scenario.
