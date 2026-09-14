%% bfbook.cls
%% Copyright 2026 Benno Falkner
%%
%% This work may be distributed and/or modified under the
%% conditions of the LaTeX Project Public License, either
%% version 1.3 of this license or (at your option) any
%% later version.
%% The latest version of this license is in
%%   http://www.latex-project.org/lppl.txt
%%
%% This work has the LPPL maintenance status 'maintained'.
%% The Current Maintainer of this work is Benno Falkner.
\NeedsTeXFormat{LaTeX2e}
\ProvidesClass{bfbook}

\RequirePackage{xkeyval}

\newif\ifbfbook@twoside
\newif\ifbfbook@copyright
\bfbook@copyrighttrue

\DeclareOptionX{paper}[a4]{\def\bfbook@paper{#1}}
\DeclareOptionX{fontsize}[11pt]{\def\bfbook@fontsize{#1}}
\DeclareOptionX{twoside}{\bfbook@twosidetrue}
\DeclareOptionX{nocopyright}{\bfbook@copyrightfalse}
\DeclareOptionX{font}[firafull]{\def\bfbook@font{#1}}
\DeclareOptionX{lang}[ngerman]{\def\bfbook@lang{#1}}

\ExecuteOptionsX{paper=a4,fontsize=11pt,font=firafull,lang=ngerman}
\ProcessOptionsX

\LoadClass[
  paper=\bfbook@paper,
  fontsize=\bfbook@fontsize,
  DIV=12,
  parskip=half,
  headings=big,
  numbers=noenddot,
  twoside=\ifbfbook@twoside true\else false\fi
]{scrbook}

\RequirePackage[top=20mm,bottom=25mm,inner=25mm,outer=20mm]{geometry}
\RequirePackage[\bfbook@lang]{babel}
\RequirePackage[\bfbook@font]{addfonts}
\RequirePackage{booktabs}
\RequirePackage{csquotes}
\RequirePackage{microtype}
\RequirePackage{tikz}
\RequirePackage{pgfplots}
\pgfplotsset{
  compat=1.18,
  every axis/.append style={font=\small},
}

\RequirePackage{xcolor}
\RequirePackage{scrlayer-scrpage}
\RequirePackage[hidelinks]{hyperref}

\pagestyle{scrheadings}
\clearpairofpagestyles
\ihead{}
\ohead{\chaptermark}
\chead{}
\ofoot[\pagemark]{\pagemark}
\cfoot{}
\automark{chapter}

\newcommand{\bfbook@copyrightnotice}{%
  \makeatletter
  \textcolor{gray!40}{\footnotesize © \the\year\ \@author}%
  \makeatother
}
\newcommand{\copyrightnotice}[1]{\renewcommand{\bfbook@copyrightnotice}{#1}}

\ifbfbook@copyright
  \ifoot[\bfbook@copyrightnotice]{\bfbook@copyrightnotice}
\fi
