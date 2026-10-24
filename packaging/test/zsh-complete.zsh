# SPDX-FileCopyrightText: 2026 grenudi
#
# SPDX-License-Identifier: GPL-3.0-or-later

# Types a command line into a real interactive zsh, presses Tab, and prints what the terminal
# showed. A wrong `_arguments` spec does not fail, it silently completes nothing, so this is the
# only honest test of the zsh completion.
#
#   zsh -f zsh-complete.zsh DIR_WITH__snapall 'snapall create --d'
zmodload zsh/zpty || exit 3
dir=$1; typed=$2
zpty zt 'zsh -f -i' || exit 4
zpty -w zt "fpath=($dir \$fpath); autoload -Uz compinit; compinit -u -d $dir/.zcompdump; autoload -Uz _snapall; compdef _snapall snapall; PS1='READY> '; setopt no_beep; zstyle ':completion:*' list-prompt ''; zstyle ':completion:*' menu no"
sleep 1; zpty -r -t zt junk 2>/dev/null
zpty -w -n zt "$typed"
sleep 0.3
zpty -w -n zt $'\t'
sleep 2
out=""; while zpty -r -t zt chunk 2>/dev/null; do out+=$chunk; done
print -r -- "$out"
zpty -d zt
