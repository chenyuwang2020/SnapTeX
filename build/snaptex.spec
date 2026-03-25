# -*- mode: python ; coding: utf-8 -*-
"""PyInstaller spec for SnapTeX."""

import os
from PyInstaller.utils.hooks import collect_data_files, collect_submodules

block_cipher = None

BUILD_DIR = os.path.dirname(os.path.abspath(SPEC))
PROJECT_ROOT = os.path.dirname(BUILD_DIR)
SRC_DIR = os.path.join(PROJECT_ROOT, 'src')
SNAPTEX_DIR = os.path.join(SRC_DIR, 'snaptex')

# Collect data files from dependencies
datas = []
datas += collect_data_files('pix2text')
datas += collect_data_files('rapidocr')
datas += collect_data_files('rapidocr_onnxruntime')
datas += collect_data_files('transformers')
datas += collect_data_files('optimum')
datas += collect_data_files('onnxruntime')

# Add SnapTeX resources
datas += [(os.path.join(SNAPTEX_DIR, 'resources'), 'snaptex/resources')]

# Add SnapTeX source files
for py_file in ['__init__.py', '__main__.py', 'app.py', 'config.py',
                'controller.py', 'workers.py', 'clipboard.py', 'ui.py']:
    src = os.path.join(SNAPTEX_DIR, py_file)
    if os.path.exists(src):
        datas += [(src, 'snaptex')]

# Hidden imports
hiddenimports = [
    'snaptex',
    'snaptex.app',
    'snaptex.config',
    'snaptex.controller',
    'snaptex.workers',
    'snaptex.clipboard',
    'snaptex.ui',
    'pix2text',
    'pix2text.latex_ocr',
    'pix2text.consts',
    'pix2text.hf_downloader',
    'rapidocr_onnxruntime',
    'onnxruntime',
    'torch',
    'transformers',
    'PIL',
    'numpy',
    'PySide6.QtWebEngineWidgets',
    'PySide6.QtWebEngineCore',
]
hiddenimports += collect_submodules('onnxruntime')
hiddenimports += collect_submodules('transformers')
hiddenimports += collect_submodules('optimum')

# Packages to EXCLUDE
excludes = [
    'torchaudio',
    'IPython',
    'jupyter',
    'notebook',
    'tkinter',
    'pytest',
    'pip',
    'PySide6.Qt3DAnimation',
    'PySide6.Qt3DCore',
    'PySide6.Qt3DExtras',
    'PySide6.Qt3DInput',
    'PySide6.Qt3DLogic',
    'PySide6.Qt3DRender',
    'PySide6.QtBluetooth',
    'PySide6.QtCharts',
    'PySide6.QtDataVisualization',
    'PySide6.QtDesigner',
    'PySide6.QtHelp',
    'PySide6.QtMultimedia',
    'PySide6.QtMultimediaWidgets',
    'PySide6.QtNfc',
    'PySide6.QtPositioning',
    'PySide6.QtQuick',
    'PySide6.QtQuickControls2',
    'PySide6.QtQuickWidgets',
    'PySide6.QtRemoteObjects',
    'PySide6.QtScxml',
    'PySide6.QtSensors',
    'PySide6.QtSerialPort',
    'PySide6.QtTest',
    'PySide6.QtTextToSpeech',
]

a = Analysis(
    [os.path.join(BUILD_DIR, 'entry.py')],
    pathex=[SRC_DIR],
    binaries=[],
    datas=datas,
    hiddenimports=hiddenimports,
    hookspath=[],
    hooksconfig={},
    runtime_hooks=[os.path.join(BUILD_DIR, 'hook_inspect.py')],
    excludes=excludes,
    win_no_prefer_redirects=False,
    win_private_assemblies=False,
    cipher=block_cipher,
    noarchive=False,
)

pyz = PYZ(a.pure, a.zipped_data, cipher=block_cipher)

exe = EXE(
    pyz,
    a.scripts,
    [],
    exclude_binaries=True,
    name='SnapTeX',
    debug=False,
    bootloader_ignore_signals=False,
    strip=False,
    upx=True,
    console=False,
    disable_windowed_traceback=False,
    argv_emulation=False,
    icon=os.path.join(SNAPTEX_DIR, 'resources', 'icon.ico'),
)

coll = COLLECT(
    exe,
    a.binaries,
    a.zipfiles,
    a.datas,
    strip=False,
    upx=True,
    upx_exclude=[],
    name='SnapTeX',
)
