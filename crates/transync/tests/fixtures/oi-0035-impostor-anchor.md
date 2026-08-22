# OI-0035 — an impostor anchor written into source HTML

<div data-sync-id="p-0003" data-block-kind="paragraph" data-order="0" data-fallback="translated" class="impostor-marker">
This raw HTML claims the id of the paragraph below it. It is content, and it
translates like any other html block — but its sync attributes are not ours.
</div>

This paragraph is the real `p-0003`: the block the raw HTML above tries to
pre-claim. In document order the impostor comes first, which is exactly the
shape the engine layer cannot defeat by itself.

<div class="jsx-habit" data-sync-id="p-0005"/>
The tag above closes itself the way XML means it, which is a JSX habit and, in
HTML content, a parse error the parser ignores: the element opens anyway, so
the closing tag below is real. Its reserved attribute makes this the composed
pane chain — the strip removes the attribute and leaves the slash behind, and
the balancer still has to keep this closer.
</div>

This paragraph is the real `p-0005`. Before ti 490d97 wave 1 the walk read the
`/` the way XML means it, refused to push the tag, and the balancer deleted the
block's own closing tag as an orphan — so the wrapper's `</div>` closed the
block instead of the wrapper, and this anchor mounted INSIDE the html block's
wrapper.
