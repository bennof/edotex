%% bfarticle.cls
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
\ProvidesClass{bfarticle}

\RequirePackage{xkeyval}

\newif\ifbfarticle@twoside
\newif\ifbfarticle@copyright
\bfarticle@copyrighttrue

\DeclareOptionX{paper}[a4]{\def\bfarticle@paper{#1}}
\DeclareOptionX{fontsize}[11pt]{\def\bfarticle@fontsize{#1}}
\DeclareOptionX{twoside}{\bfarticle@twosidetrue}
\DeclareOptionX{nocopyright}{\bfarticle@copyrightfalse}
\DeclareOptionX{font}[firafull]{\def\bfarticle@font{#1}}
\DeclareOptionX{lang}[ngerman]{\def\bfarticle@lang{#1}}

\ExecuteOptionsX{paper=a4,fontsize=11pt,font=firafull,lang=ngerman}
\ProcessOptionsX

\LoadClass[
  paper=\bfarticle@paper,
  fontsize=\bfarticle@fontsize,
  DIV=12,
  parskip=half,
  headings=normal,
  twoside=\ifbfarticle@twoside true\else false\fi
]{scrartcl}

\RequirePackage[top=20mm,bottom=25mm,inner=25mm,outer=20mm]{geometry}
\RequirePackage[\bfarticle@lang]{babel}
\RequirePackage[\bfarticle@font]{addfonts}
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
\ohead{\headmark}
\chead{}
\ofoot[\pagemark]{\pagemark}
\cfoot{}
\automark[subsection]{section}

\newcommand{\bfarticle@copyrightnotice}{%
  \textcolor{gray!40}{\footnotesize © \the\year\ \@author}%
}
\newcommand{\copyrightnotice}[1]{\renewcommand{\bfarticle@copyrightnotice}{#1}}

\ifbfarticle@copyright
  \ifoot[\bfarticle@copyrightnotice]{\bfarticle@copyrightnotice}
\fi
