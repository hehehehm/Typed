#!/usr/bin/env bash
# Run in an empty directory. Fetch English (and Russian) labels of brands, companies, software, games, websites,
# products from Wikidata (CC0). Output: wd_<name>.tsv (label@lang, sitelinks). Then: python3 tools/brands/build.py <dir> <full-form dir> <repo>.
set -u
q() { # name class minsitelinks path
  local name=$1 cls=$2 min=$3 path=${4:-"wdt:P31/wdt:P279?"}
  local query="SELECT ?l ?s WHERE { ?i $path wd:$cls ; wikibase:sitelinks ?s . FILTER(?s >= $min) ?i rdfs:label ?l . FILTER(lang(?l) = 'en' || lang(?l) = 'ru') }"
  for try in 1 2 3; do
    if curl -sS -m 300 -G https://query.wikidata.org/sparql -H 'Accept: text/tab-separated-values' \
      -H 'User-Agent: TypedBrandList/0.1 (offline word list build; contact via github hehehehm/typed)' \
      --data-urlencode "query=$query" -o "wd_$name.tsv.tmp" && head -1 "wd_$name.tsv.tmp" | grep -q '?l'; then
      mv "wd_$name.tsv.tmp" "wd_$name.tsv"; echo "$name: $(($(wc -l < wd_$name.tsv)-1))"; return; fi
    sleep 5
  done
  echo "$name: FAILED"; head -c 300 "wd_$name.tsv.tmp"
}
q brand Q431289 2 "wdt:P31/wdt:P279*"
q business Q4830453 8
q company Q783794 6
q enterprise Q6881511 6
q software Q7397 6
q mobileapp Q620615 2
q videogame Q7889 8
q website Q35127 5
q onlineservice Q19967801 2
q smartphone Q19723451 2
q automodel Q3231690 6
q automaker Q786820 3
q carbrand Q44294 2
q crypto Q13479982 3
q product Q2424752 6
q os Q9135 3
q webbrowser Q6368 2
q prog_lang Q9143 8
q game_franchise Q7058673 5
q social_network Q3220391 2
q fashion_house Q3661311 2
q restaurant_chain Q18534 3
q retail_chain Q507619 3
q beverage_brand Q2832891 1
q airline Q46970 6
