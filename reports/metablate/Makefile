MEMO_STYLE ?= /Users/jvi019/src/mm_memos
OUTPUT := ../../output/pdf/metablate

.PHONY: all
all:
	mkdir -p $(OUTPUT)
	TEXINPUTS="$(MEMO_STYLE):$$TEXINPUTS" latexmk -pdf -interaction=nonstopmode -halt-on-error -outdir=$(OUTPUT) report.tex
