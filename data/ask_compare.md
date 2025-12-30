You are an expert in construction description.
Your speciality is only windows, doors and radiators, if present.
It is necessary to compare in details two different descriptions of the same object, but in different time - old and new.
 the quantity, material, condition, completeness and stage of installation of windows, doors and radiators.

Response format (JSON only, no other text):
{{
"description": "General and complete description of the object",
"windows": "Detailed information about windows only",
"doors": "Detailed information about doors only",
"radiators": "Detailed information about radiators only",
}}

Old:
{
  "description": "Unfinished concrete structure with large window openings but no installed windows, doors, or radiators. The space shows exposed concrete walls and incomplete openings, indicating early construction stage with no fixtures present.",
  "windows": "No windows installed; three concrete window openings present (2.5m height x 1.8m width each). Openings lack frames, glazing, and finishing. Condition: unfinished raw concrete. Installation stage: structural openings only, no fixtures mounted.",
  "doors": "No doors present; no door frames, jambs, or openings prepared for installation. Condition: incomplete construction phase with no door-related elements visible.",
  "radiators": "No radiators present; no heating infrastructure visible in the space. No mounting points, piping, or equipment observed in the unfinished concrete structure."
}

New:
{
  "description": "Finished interior space with installed windows and radiators, but no visible doors. The room features painted walls, functional window coverings, and operational heating units, indicating a completed installation stage for present elements.",
  "windows": "Three installed windows (1.5m x 1.2m each) with aluminum frames. Upper sections feature beige roller blinds (partially extended, showing minor wear), lower sections have frosted glass panes. Condition: fully operational with intact frames. Installation stage: complete with functional hardware and glazing.",
  "doors": "No doors present; no door frames, jambs, or openings visible in the space. Walls are fully painted with no preparation for door installation observed.",
  "radiators": "Two cast-iron radiators (1.2m length each) mounted beneath windows. Light beige paint with minor surface oxidation. Condition: fully functional with visible connection pipes and valves. Installation stage: complete with operational heating system integration."
}
