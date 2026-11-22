#!/bin/sh
set -e
curl -fLO https://downloads.tatoeba.org/exports/per_language/rus/rus_sentences.tsv.bz2
curl -fLO https://downloads.tatoeba.org/exports/per_language/eng/eng_sentences.tsv.bz2
curl -fLO https://dumps.wikimedia.org/ruwiki/latest/ruwiki-latest-pages-articles1.xml-p1p224167.bz2
curl -fLO https://dumps.wikimedia.org/enwiki/latest/enwiki-latest-pages-articles1.xml-p1p41242.bz2
