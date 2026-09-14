\NeedsTeXFormat{LaTeX2e}
\ProvidesClass{edoxbook}[2025/09/28 EDOX book class]

\LoadClass{bfbook}
\RequirePackage{edoxbase}
\automark{section}

\newcounter{exercise}[subsection]
\newcounter{subexercise}[exercise]

\renewcommand*\theexercise{\thesubsection.\arabic{exercise}}

% ------------------------------------------------------------
% Helper macro: remove the first N characters of a string
%
% Usage:
%   \StripLeft{<N>}{<string>}{\resultmacro}
%
% Example:
%   \StripLeft{3}{ABCDEF}{\Result}
%   -> \Result contains DEF
%
% Implementation uses pure TeX token processing.
% ------------------------------------------------------------
\makeatletter

\newcommand{\StripLeft}[3]{%
  \edef\edox@tmp{#2}%
  \expandafter\edox@stripleft\edox@tmp\relax{#1}{#3}%
}

\def\edox@stripleft#1#2\relax#3#4{%
  \ifnum#3>0
    % Remove one token and recurse
    \expandafter\edox@stripleft\expandafter#2\relax{\the\numexpr#3-1\relax}{#4}%
  \else
    % Store result in target macro
    \expandafter\def\expandafter#4\expandafter{#1#2}%
  \fi
}

\makeatother

% ------------------------------------------------------------
% Import a chapter from a directory
%
% Usage:
%   \importchapter[<stripN>]{<path>}
%
% Example:
%   \importchapter[5]{chapters/01_intro}
%
% Behaviour:
%   1. Optional: remove first N characters from path to form title
%   2. Create chapter with derived title
%   3. Temporarily set graphics path to <path>/figures/
%   4. Input <path>/content.tex
% ------------------------------------------------------------
\newcommand{\importchapter}[2][0]{%
  % Determine chapter title
  \StripLeft{#1}{#2}{\edox@ChapterName}%
  
  % Create chapter
  \chapter{\edox@ChapterName}

  % Input chapter with local graphics path
  \begingroup
    \graphicspath{{#2/figures/}}
    \input{#2/content.tex}
  \endgroup
}


