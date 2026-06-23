"""Tests for the static render mode and build-time renderers.

These exercise the `mode` and `renderers` keywords on `to_html`, which delegate
to carve-rs `Mode::Static` / `StaticRenderers`. They import the real native
module, so they only pass once the wheel is built (maturin develop / build).
"""

import carve
import pytest

DETAILS = '::: details "x"\nbody\n:::'
MERMAID = "``` mermaid\ngraph TD; A-->B;\n```"
MATH = "``` math\n\\int_0^1 x^2 dx\n```"


def test_details_static_flattens_to_section():
    # Static mode flattens the interactive <details> to a self-contained
    # <section> (no client toggle needed for print / PDF / archival).
    out = carve.to_html(DETAILS, extensions=["details"], mode="static")
    assert '<section class="details">' in out
    assert "<details>" not in out


def test_details_interactive_keeps_details_element():
    out = carve.to_html(DETAILS, extensions=["details"], mode="interactive")
    assert "<details>" in out
    assert '<section class="details">' not in out


def test_mode_omitted_equals_interactive():
    # Omitting `mode` must be identical to mode="interactive" (non-breaking).
    default_out = carve.to_html(DETAILS, extensions=["details"])
    interactive_out = carve.to_html(DETAILS, extensions=["details"], mode="interactive")
    assert default_out == interactive_out
    assert "<details>" in default_out


def test_mermaid_static_no_renderer_degrades_to_source():
    # No mermaid renderer supplied: the static path degrades to a source
    # <pre><code> block (never blank).
    out = carve.to_html(MERMAID, extensions=["fenced_render"], mode="static")
    assert "<pre" in out and "<code" in out
    assert "<svg>" not in out


def test_mermaid_static_with_renderer_injects_output():
    out = carve.to_html(
        MERMAID,
        extensions=["fenced_render"],
        mode="static",
        renderers={"mermaid": lambda s: "<svg>" + s + "</svg>"},
    )
    assert "<svg>" in out
    assert "graph TD" in out


def test_math_renderer_called_with_display_true_for_block():
    calls = []

    def render_math(tex, display):
        calls.append((tex, display))
        return f"<math d={display}>{tex}</math>"

    out = carve.to_html(
        MATH, extensions=["math_block"], mode="static", renderers={"math": render_math}
    )
    # The math renderer must receive the TeX source and display=True for a
    # block / display-math construct.
    assert len(calls) == 1
    tex, display = calls[0]
    assert display is True
    assert "\\int_0^1" in tex
    assert "<math d=True>" in out


def test_renderer_that_raises_falls_back_to_source():
    # A renderer that raises must degrade to source, not propagate or blank.
    def boom(_s):
        raise RuntimeError("boom")

    out = carve.to_html(
        MERMAID, extensions=["fenced_render"], mode="static", renderers={"mermaid": boom}
    )
    assert "A--&gt;B" in out  # source preserved, HTML-escaped
    assert "<svg>" not in out


def test_failing_renderer_fallback_escapes_html():
    # carve-rs emits a present renderer's return value verbatim, so the
    # failure fallback must HTML-escape the source - a raising renderer must
    # not let document source inject raw HTML (XSS floor).
    src = "``` mermaid\n<img src=x onerror=alert(1)>\n```"

    def boom(_s):
        raise RuntimeError("boom")

    out = carve.to_html(
        src, extensions=["fenced_render"], mode="static", renderers={"mermaid": boom}
    )
    assert "<img src=x onerror=alert(1)>" not in out
    assert "&lt;img src=x onerror=alert(1)&gt;" in out


def test_failing_renderer_returns_non_string_escapes_html():
    # A non-string return also falls back to escaped source, never raw.
    src = "``` mermaid\n<b>x</b>\n```"
    out = carve.to_html(
        src,
        extensions=["fenced_render"],
        mode="static",
        renderers={"mermaid": lambda _s: 123},  # non-string return
    )
    assert "<b>x</b>" not in out
    assert "&lt;b&gt;x&lt;/b&gt;" in out


def test_bogus_mode_raises_value_error():
    with pytest.raises(ValueError):
        carve.to_html("x", mode="bogus")


def test_unknown_renderer_key_raises_value_error():
    with pytest.raises(ValueError):
        carve.to_html("x", renderers={"nope": lambda s: s})


def test_to_html_with_extensions_supports_static_mode():
    out = carve.to_html_with_extensions(DETAILS, ["details"], mode="static")
    assert '<section class="details">' in out
