"""Type stubs for the `carve` native binding (PyO3 over carve-rs)."""

from typing import Callable, Dict, List, Optional, Union

__version__: str

# A static-render callable. Diagram renderers (mermaid / chart) take the
# construct source and return HTML: ``(str) -> str``. The math renderer takes
# the TeX source and a ``display`` flag (``True`` for block/display math,
# ``False`` for inline) and returns HTML: ``(str, bool) -> str``.
Renderer = Union[Callable[[str], str], Callable[[str, bool], str]]

def to_html(
    source: str,
    extensions: Optional[List[str]] = None,
    mode: str = "interactive",
    renderers: Optional[Dict[str, Renderer]] = None,
) -> str:
    """Convert Carve source to HTML.

    With no ``extensions`` (or an empty list) this is the core renderer,
    identical to carve-rs ``to_html``. Pass extension names to enable opt-in
    behavior. Raises ``ValueError`` for an unknown extension name.

    ``mode`` is ``"interactive"`` (default) or ``"static"``; any other value
    raises ``ValueError``. Static mode flattens interactive constructs to
    self-contained HTML (no client scripts).

    ``renderers`` is an optional dict of build-time renderer callables consulted
    only on the static HTML path. Keys: ``"mermaid"`` / ``"chart"`` (callables
    ``(str) -> str``) and ``"math"`` (callable ``(str, bool) -> str``). An
    unknown key raises ``ValueError``. A missing renderer degrades that
    construct to its source (never blank). A renderer that raises or returns a
    non-string also degrades to source.
    """
    ...

def to_html_with_extensions(
    source: str,
    extensions: List[str],
    mode: str = "interactive",
    renderers: Optional[Dict[str, Renderer]] = None,
) -> str:
    """Convert Carve source to HTML with an explicit extension list.

    Supports the same ``mode`` / ``renderers`` keywords as :func:`to_html`.
    """
    ...

def to_markdown(source: str, extensions: Optional[List[str]] = None) -> str:
    """Convert Carve source to Markdown (inherently static; no ``mode``)."""
    ...

def to_plain_text(source: str, extensions: Optional[List[str]] = None) -> str:
    """Convert Carve source to plain text (inherently static; no ``mode``)."""
    ...

def to_ansi(source: str, extensions: Optional[List[str]] = None) -> str:
    """Convert Carve source to ANSI-colored text (inherently static; no ``mode``)."""
    ...

def extensions() -> List[str]:
    """Return the list of supported extension names."""
    ...
