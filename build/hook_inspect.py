"""Runtime hook for PyInstaller: patch inspect.getsource for frozen env."""

import inspect

_original_getsource = inspect.getsource
_original_getsourcelines = inspect.getsourcelines


def _patched_getsource(obj):
    try:
        return _original_getsource(obj)
    except (OSError, TypeError):
        # Return a minimal valid function source so callers that parse it won't crash
        return "def _frozen_placeholder():\n    pass\n"


def _patched_getsourcelines(obj):
    try:
        return _original_getsourcelines(obj)
    except (OSError, TypeError):
        return (["def _frozen_placeholder():\n", "    pass\n"], 0)


inspect.getsource = _patched_getsource
inspect.getsourcelines = _patched_getsourcelines
