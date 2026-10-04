//! **Appliance care library** — what to do to keep each common appliance
//! working: how to look after it, the routine jobs and how often, and how long
//! one usually lasts. An appliance is matched by its name, kind and make; the
//! owner can accept the suggestion as the appliance's care instructions and
//! routine schedule, then edit either.

use serde::Serialize;

pub struct CareTask {
    pub title: &'static str,
    pub description: &'static str,
    pub cadence_days: i32,
    pub priority: &'static str,
}

pub struct Care {
    pub key: &'static str,
    pub label: &'static str,
    /// Matched against the appliance's name, in lowercase.
    pub keywords: &'static [&'static str],
    pub life_years: i32,
    /// Plain lines; each starts on its own line.
    pub instructions: &'static str,
    pub tasks: &'static [CareTask],
}

const fn t(
    title: &'static str,
    description: &'static str,
    cadence_days: i32,
    priority: &'static str,
) -> CareTask {
    CareTask {
        title,
        description,
        cadence_days,
        priority,
    }
}

pub const LIBRARY: &[Care] = &[
    Care {
        key: "water_heater",
        label: "Water heater",
        keywords: &["water heater", "hot water", "boiler"],
        life_years: 12,
        instructions: "Keep the thermostat at 120°F.\nKeep the area around it clear.\nIf you see a puddle or smell gas, shut off the water and gas and call for service.\nDo not store anything flammable nearby.",
        tasks: &[
            t("Flush the tank", "Drain a few gallons from the drain valve until the water runs clear to clear sediment.", 365, "normal"),
            t("Test the pressure relief valve", "Lift the lever briefly; water should flow and stop. Replace the valve if it drips afterward.", 365, "normal"),
            t("Check the anode rod", "Inspect it every few years and replace it when it is mostly eaten away.", 1095, "low"),
        ],
    },
    Care {
        key: "hvac",
        label: "Furnace and air conditioner",
        keywords: &["hvac", "furnace", "air conditioner", "a/c", "heat pump", "air handler", "condenser"],
        life_years: 15,
        instructions: "Change the filter on schedule; a clogged filter is the most common cause of failure.\nKeep the outdoor unit clear of leaves, grass and snow for 2 feet on every side.\nKeep vents and returns uncovered.\nIf the unit short-cycles, ices over or makes grinding noises, turn it off and call for service.",
        tasks: &[
            t("Replace the air filter", "Replace with the size printed on the old filter.", 90, "normal"),
            t("Spring air conditioner tune-up", "Clean the coil, check refrigerant, test the capacitor and clear the condensate drain.", 365, "normal"),
            t("Fall furnace tune-up", "Inspect the burners and heat exchanger, test safety controls and check the flue.", 365, "normal"),
            t("Clear the condensate drain line", "Flush the drain line with a cup of vinegar to prevent clogs.", 180, "low"),
        ],
    },
    Care {
        key: "dishwasher",
        label: "Dishwasher",
        keywords: &["dishwasher"],
        life_years: 10,
        instructions: "Scrape plates, but do not rinse them completely; detergent needs something to work on.\nRun hot water at the sink first so the cycle starts hot.\nLeave the door ajar after a cycle to dry the inside.\nDo not overload the racks or block the spray arms.",
        tasks: &[
            t("Clean the filter", "Remove the bottom filter, rinse away food and put it back.", 30, "low"),
            t("Run a cleaning cycle", "Run an empty hot cycle with a dishwasher cleaner or a cup of vinegar.", 90, "low"),
            t("Check the door seal and supply line", "Look for cracks, mold and leaks under the unit.", 365, "normal"),
        ],
    },
    Care {
        key: "refrigerator",
        label: "Refrigerator",
        keywords: &["refrigerator", "fridge", "freezer"],
        life_years: 13,
        instructions: "Keep it at 37°F and the freezer at 0°F.\nLeave a few inches behind it for airflow.\nWipe the door seals so they close tightly.\nIf the water filter light is on, replace the filter.",
        tasks: &[
            t("Vacuum the condenser coils", "Pull the unit out or remove the grille and vacuum the coils.", 180, "normal"),
            t("Replace the water filter", "Replace the filter if the unit has one.", 180, "low"),
            t("Check door seals", "Close a dollar bill in the door; it should resist being pulled out.", 365, "low"),
        ],
    },
    Care {
        key: "range",
        label: "Range, oven and cooktop",
        keywords: &["range", "oven", "stove", "cooktop"],
        life_years: 13,
        instructions: "Wipe spills when the surface has cooled.\nDo not use foil to line the oven bottom.\nOn a gas range, a yellow or orange flame means it needs service.\nIf you smell gas, leave, do not touch switches and call the gas company.",
        tasks: &[
            t("Deep clean the oven", "Use the self-clean cycle or an oven cleaner with the room ventilated.", 180, "low"),
            t("Check burners and igniters", "Make sure every burner lights evenly and the igniters click.", 365, "normal"),
        ],
    },
    Care {
        key: "microwave",
        label: "Microwave and range hood",
        keywords: &["microwave", "range hood", "vent hood"],
        life_years: 9,
        instructions: "Do not run it empty.\nWipe the inside with a damp cloth.\nWash the grease filter in hot soapy water.",
        tasks: &[t("Wash the grease filter", "Remove the filter and soak it in hot soapy water.", 90, "low")],
    },
    Care {
        key: "washer",
        label: "Washing machine",
        keywords: &["washer", "washing machine"],
        life_years: 10,
        instructions: "Leave the door ajar between loads to keep it from smelling.\nUse high-efficiency detergent and do not overdose it.\nTurn the water supply off if you will be away for a long time.\nDo not overload the drum.",
        tasks: &[
            t("Run a washer cleaning cycle", "Run an empty hot cycle with washer cleaner.", 60, "low"),
            t("Inspect the fill hoses", "Look for bulges, cracks and leaks; replace braided hoses every five years.", 365, "high"),
            t("Clean the drain pump filter", "Open the access panel and clear the filter.", 180, "low"),
        ],
    },
    Care {
        key: "dryer",
        label: "Dryer",
        keywords: &["dryer"],
        life_years: 13,
        instructions: "Clean the lint screen before every load.\nNever run it when you are out of the house.\nKeep the vent hose short, straight and metal.\nIf clothes take more than one cycle to dry, check the vent.",
        tasks: &[
            t("Clean the dryer vent", "Disconnect the duct and clear lint from the duct and the outside hood. A clogged vent is a fire risk.", 365, "high"),
            t("Wash the lint screen", "Wash the lint screen with soap and water to remove softener film.", 180, "low"),
        ],
    },
    Care {
        key: "disposal",
        label: "Garbage disposal",
        keywords: &["disposal"],
        life_years: 12,
        instructions: "Run cold water while it works and for 15 seconds after.\nNo grease, bones, coffee grounds, fibrous peels or pasta.\nIf it hums but does not spin, turn it off and use the hex key underneath to free it.\nNever put your hand inside.",
        tasks: &[t("Clean and deodorize the disposal", "Grind ice and citrus peel, then flush with cold water.", 60, "low")],
    },
    Care {
        key: "detector",
        label: "Smoke and CO detectors",
        keywords: &["smoke", "carbon monoxide", "co detector", "detector", "alarm"],
        life_years: 10,
        instructions: "Test each alarm monthly.\nReplace the batteries every year, or when it chirps.\nReplace smoke alarms every 10 years and CO alarms by the date printed on the back.\nNever disable one; open a window instead.",
        tasks: &[
            t("Test all alarms", "Press the test button on every alarm and note any that fail.", 30, "high"),
            t("Replace the batteries", "Replace the batteries in every alarm that uses them.", 365, "high"),
        ],
    },
    Care {
        key: "thermostat",
        label: "Thermostat",
        keywords: &["thermostat"],
        life_years: 10,
        instructions: "Set it and leave it; large swings cost more than they save.\nKeep sunlight and drafts off it.\nReplace the batteries if it has them and the screen dims.",
        tasks: &[t("Check the thermostat batteries and schedule", "Replace batteries if used and confirm the schedule is right.", 365, "low")],
    },
    Care {
        key: "garage_door",
        label: "Garage door opener",
        keywords: &["garage door", "garage opener"],
        life_years: 15,
        instructions: "Test the auto-reverse by laying a 2x4 under the door; it must reverse on contact.\nKeep the photo-eye sensors clean and aligned.\nDo not use the opener if a spring is broken.",
        tasks: &[
            t("Test the auto-reverse and sensors", "Lay a 2x4 flat under the door; it must reverse. Wipe the sensors.", 180, "high"),
            t("Lubricate the door", "Lubricate the rollers, hinges and springs with garage door lubricant.", 365, "low"),
        ],
    },
    Care {
        key: "sump_pump",
        label: "Sump pump",
        keywords: &["sump"],
        life_years: 10,
        instructions: "Pour a bucket of water in the pit to check it turns on and drains.\nKeep the pit free of debris.\nConsider a battery backup in a storm-prone area.",
        tasks: &[t("Test the sump pump", "Pour in water until the float lifts and confirm it pumps out.", 90, "high")],
    },
    Care {
        key: "ceiling_fan",
        label: "Ceiling fan",
        keywords: &["ceiling fan", "fan"],
        life_years: 15,
        instructions: "Run it counter-clockwise in summer and clockwise in winter.\nDust the blades.\nTurn it off if it wobbles badly.",
        tasks: &[t("Dust the blades and tighten the fixtures", "Dust the blades and tighten the screws on the mount and blades.", 180, "low")],
    },
];

/// The library entry for an appliance, by its name then kind.
pub fn matching(name: &str, kind: &str) -> Option<&'static Care> {
    let name = name.to_lowercase();
    LIBRARY
        .iter()
        .find(|c| c.keywords.iter().any(|k| name.contains(k)))
        .or_else(|| {
            let kind = kind.to_lowercase();
            LIBRARY.iter().find(|c| c.key == kind)
        })
}

pub fn by_key(key: &str) -> Option<&'static Care> {
    LIBRARY.iter().find(|c| c.key == key)
}

#[derive(Serialize, schemars::JsonSchema)]
pub struct CareEntry {
    pub key: String,
    pub label: String,
}

pub fn entries() -> Vec<CareEntry> {
    LIBRARY
        .iter()
        .map(|c| CareEntry {
            key: c.key.into(),
            label: c.label.into(),
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn appliances_are_matched_by_name_then_kind() {
        assert_eq!(
            matching("Water heater (garage)", "appliance").unwrap().key,
            "water_heater"
        );
        assert_eq!(
            matching("Central AC and furnace", "hvac").unwrap().key,
            "hvac"
        );
        assert_eq!(
            matching("Kitchen dishwasher", "appliance").unwrap().key,
            "dishwasher"
        );
        assert_eq!(matching("Whatever", "hvac").unwrap().key, "hvac");
        assert!(matching("Whatever", "other").is_none());
    }

    #[test]
    fn every_entry_is_complete() {
        for c in LIBRARY {
            assert!(!c.instructions.is_empty() && c.life_years > 0, "{}", c.key);
            for t in c.tasks {
                assert!(t.cadence_days > 0, "{} / {}", c.key, t.title);
                assert!(
                    matches!(t.priority, "low" | "normal" | "high" | "urgent"),
                    "{} / {}",
                    c.key,
                    t.title
                );
            }
        }
        let mut keys: Vec<_> = LIBRARY.iter().map(|c| c.key).collect();
        keys.sort();
        keys.dedup();
        assert_eq!(keys.len(), LIBRARY.len(), "keys are unique");
    }
}
