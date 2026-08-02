# CSV Roster export always renders in English, regardless of Display language

ADR-0026 established a one-off CSV Roster export for sharing with reviewers or offline archiving. With localization added, we decided the export's column headers and content always render in English, never following the exporting user's selected Display language. The export is a fixed, parseable schema meant to be opened by any reviewer or tool regardless of the exporting install's language setting — consistency across every export matters more than the exporting user's personal comfort reading their own file.
