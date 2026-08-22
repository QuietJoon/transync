# OI-0035 — an impostor anchor written into source HTML

<div data-sync-id="p-0003" data-block-kind="paragraph" data-order="0" data-fallback="translated" class="impostor-marker">
This raw HTML claims the id of the paragraph below it. It is content, and it
translates like any other html block — but its sync attributes are not ours.
</div>

This paragraph is the real `p-0003`: the block the raw HTML above tries to
pre-claim. In document order the impostor comes first, which is exactly the
shape the engine layer cannot defeat by itself.
