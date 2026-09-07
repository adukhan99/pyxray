# pyxray — build, install and look at things.
#
# Everything lands under ./target and ./python; nothing is written to $HOME.

CARGO ?= cargo
PYTHON ?= python3
WIDTH ?= 112
EXAMPLE ?= examples/analysis.py

.PHONY: help build release test check fmt clippy install-ext sheet demo clean

help:
	@printf 'targets:\n'
	@printf '  build        debug build of the workspace\n'
	@printf '  release      optimised build (this is the one you want)\n'
	@printf '  test         cargo tests plus the Python round-trip\n'
	@printf '  check        fmt + clippy + test\n'
	@printf '  install-ext  copy the built extension next to the Python package\n'
	@printf '  sheet        regenerate docs/contact-sheet.html\n'
	@printf '  demo         render every theme to docs/gallery/\n'

build:
	$(CARGO) build

release:
	$(CARGO) build --release
	@printf '\nbinary: target/release/pyx (%s)\n' "$$(du -h target/release/pyx | cut -f1)"

test:
	$(CARGO) test
	PYTHONPATH=python $(PYTHON) -m pytest python/tests -q 2>/dev/null || \
	  PYTHONPATH=python $(PYTHON) python/tests/test_roundtrip.py

check: fmt clippy test

fmt:
	$(CARGO) fmt --all

clippy:
	$(CARGO) clippy --all-targets -- -D warnings

install-ext: release
	cp target/release/lib_pyxray.so python/pyxray/_pyxray.abi3.so
	@printf 'installed python/pyxray/_pyxray.abi3.so\n'

sheet: build
	@mkdir -p build
	./target/debug/pyx --contact-sheet build/contact-sheet.html --width $(WIDTH) $(EXAMPLE)

demo: build
	@mkdir -p build/gallery
	@for t in blueprint neon paper carbon amber ansi mono; do \
	  ./target/debug/pyx -t $$t -l dashboard -f svg -w $(WIDTH) -o build/gallery/$$t.svg $(EXAMPLE); \
	  ./target/debug/pyx -t $$t -l dashboard -f html -w $(WIDTH) -o build/gallery/$$t.html $(EXAMPLE); \
	done
	@printf 'wrote build/gallery/\n'

clean:
	$(CARGO) clean

.PHONY: lookdev audit
lookdev: release
	$(PYTHON) tools/render_fragments.py
	$(PYTHON) tools/build_lookdev.py build/lookdev
	@printf 'wrote build/lookdev/contact-sheet.html\n'

audit:
	@$(PYTHON) tools/contrast_audit.py
