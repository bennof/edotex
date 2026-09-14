\NeedsTeXFormat{LaTeX2e}
\ProvidesClass{edoxarticle}[2025/09/28 EDOX article class]

\LoadClass[nocopyright]{bfarticle}
\RequirePackage{edoxbase}
\automark{section}

\let\edo@oldsection\section
\RenewDocumentCommand{\section}{s m}{%
  \IfBooleanTF{#1}
    {\edo@oldsection*{#2}\markright{#2}}%
    {\edo@oldsection{#2}\markright{#2}}%
}

%\newcounter{exercise}[subsection]
%\newcounter{subexercise}[exercise]

\renewcommand*\theexercise{\thesubsection.\arabic{exercise}}