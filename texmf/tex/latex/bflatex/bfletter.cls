%% bfletter.cls
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
\ProvidesClass{bfletter}

\RequirePackage{xkeyval}

\newif\ifbfletter@twoside
\newif\ifbfletter@digital
\newif\ifbfletter@foldmarks
\bfletter@foldmarkstrue

\DeclareOptionX{paper}[a4]{\def\bfletter@paper{#1}}
\DeclareOptionX{fontsize}[11pt]{\def\bfletter@fontsize{#1}}
\DeclareOptionX{font}[fira]{\def\bfletter@font{#1}}
\DeclareOptionX{lang}[ngerman]{\def\bfletter@lang{#1}}
\DeclareOptionX{sender}[]{\def\bfletter@sender{#1}}
\DeclareOptionX{twoside}{\bfletter@twosidetrue}
\DeclareOptionX{digital}{\bfletter@digitaltrue}
\DeclareOptionX{nofoldmarks}{\bfletter@foldmarksfalse}

\ExecuteOptionsX{paper=a4,fontsize=11pt,font=fira,lang=ngerman,sender=}
\ProcessOptionsX

\LoadClass[
  paper=\bfletter@paper,
  fontsize=\bfletter@fontsize,
  parskip=half,
  fromalign=right,
  fromphone=true,
  fromemail=true,
  fromrule=false,
  backaddress=plain,
  foldmarks=\ifbfletter@foldmarks true\else false\fi,
  addrfield=true,
  subject=untitled,
  twoside=\ifbfletter@twoside true\else false\fi
]{scrlttr2}

\RequirePackage[left=20mm,right=20mm]{geometry}
\RequirePackage[\bfletter@lang]{babel}
\RequirePackage[\bfletter@font]{addfonts}
\RequirePackage{microtype}
\RequirePackage{datetime2}
\DTMsetdatestyle{iso}
\RequirePackage{graphicx}

\newkomavar{fromstreet}
\newkomavar{fromcity}
\newkomavar{fromcountry}

\setkomafont{subject}{\bfseries}
\setkomafont{fromname}{\bfseries}
\renewcommand*{\raggedsignature}{\raggedright}

\setplength{firstheadvpos}{14mm}

\setkomavar{firsthead}{%
  \hbox to \textwidth{%
    \hfill
    \vbox to 0pt{%
      \hbox{{\bfseries\usekomavar{fromname}}}%
      \kern1.5ex
      \hbox{\usekomavar{fromstreet}}%
      \hbox{\usekomavar{fromcity}}%
      \hbox{\usekomavar{fromcountry}}%
      \kern1.5ex
      \hbox{\usekomavar{fromphone}}%
      \hbox{\usekomavar{fromemail}}%
      \vss
    }%
  }%
}

\AtBeginDocument{%
  \setkomavar{signature}{\usekomavar{fromname}}%
  \ifx\bfletter@sender\empty\else
    \IfFileExists{sender-\bfletter@sender.tex}{%
      \input{sender-\bfletter@sender.tex}%
    }{%
      \PackageWarning{bfletter}{sender file 'sender-\bfletter@sender.tex' not found}%
    }%
  \fi
  \ifbfletter@digital
    \edef\old@signature{%
      \expandafter\noexpand\csname scr@kv@signature\endcsname
    }%
    \setkomavar{signature}{%
      \IfFileExists{signature.png}{%
        \includegraphics[height=1.5cm]{signature.png}\\%
      }{}%
      \old@signature%
    }%
  \fi
}