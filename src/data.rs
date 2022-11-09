use serde_json::Map;
use serde_json::Value;
use std::time::Instant;

const MA_SEC_TO_MA_HOUR: f64 = 1.0 / 3600.0;

trait InsertJson {
    fn insert_json(&self, map: &mut Map<String, Value>, key: &str);
}

impl InsertJson for Option<String> {
    fn insert_json(&self, map: &mut Map<String, Value>, key: &str) {
        if let Some(v) = self {
            map.insert(key.to_string(), Value::String(v.clone()));
        }
    }
}

impl InsertJson for Option<isize> {
    fn insert_json(&self, map: &mut Map<String, Value>, key: &str) {
        if let Some(v) = self {
            let v = *v;
            map.insert(key.to_string(), Value::Number(v.into()));
        }
    }
}

impl InsertJson for Option<f32> {
    fn insert_json(&self, map: &mut Map<String, Value>, key: &str) {
        if let Some(v) = self {
            let v = serde_json::value::Number::from_f64(*v as f64).expect("Error convert to  f64 json number");
            map.insert(key.to_string(), Value::Number(v));
        }
    }
}

impl InsertJson for usize {
    fn insert_json(&self, map: &mut Map<String, Value>, key: &str) {
        map.insert(key.to_string(), Value::Number((*self).into()));
    }
}

#[derive(Debug, Default)]
struct AveragingCollector<T> {
    sum: T,
    count: usize,
}

impl<T> AveragingCollector<T>
where
    T: std::ops::AddAssign + AsF64 + Default,
{
    fn collect(&mut self, value: Option<T>) {
        if let Some(v) = value {
            self.sum += v;
            self.count += 1;
        }
    }

    fn insert_json(&self, map: &mut Map<String, Value>, key: &str) {
        if self.count > 0 {
            let mut val: f64 = self.sum.as_f64() / self.count as f64;
            val = (val * 100.0).round() / 100.0;
            let json_val = serde_json::value::Number::from_f64(val);
            map.insert(key.to_string(), Value::Number(json_val.expect("Unable to make json val from f64")));
        }
    }
}

#[derive(Debug, Default)]
struct IntegratingCollector<T> {
    sum: T,
    set: bool,
    clamp_min: Option<T>,
    clamp_max: Option<T>,
    factor: T,
}

impl<T> IntegratingCollector<T>
where
    T: std::ops::AddAssign + AsF64 + Default + std::cmp::PartialOrd + Copy,
{
    fn collect(&mut self, value: Option<T>) {
        if let Some(v) = value {
            self.sum += v;
            self.set = true;

            if let Some(clamp_min) = self.clamp_min {
                if self.sum < clamp_min {
                    self.sum = clamp_min;
                }
            }
            if let Some(clamp_max) = self.clamp_max {
                if self.sum > clamp_max {
                    self.sum = clamp_max;
                }
            }
        }
    }

    fn insert_json(&self, map: &mut Map<String, Value>, key: &str) {
        if self.set {
            let mut val: f64 = self.sum.as_f64() * self.factor.as_f64();
            val = (val * 1000.0).round() / 1000.0;
            let json_val = serde_json::value::Number::from_f64(val);
            map.insert(key.to_string(), Value::Number(json_val.expect("Unable to make json val from f64")));
        }
    }

    fn reset(&mut self) {
        self.sum = Default::default();
        self.set = false;
    }
}

trait AsF64 {
    fn as_f64(&self) -> f64;
}

impl AsF64 for isize {
    fn as_f64(&self) -> f64 {
        (*self) as f64
    }
}

impl AsF64 for f64 {
    fn as_f64(&self) -> f64 {
        *self
    }
}

#[derive(Debug, Default)]
pub struct Collector {
    main_battery_voltage: AveragingCollector<isize>,
    channel2_battery_voltage: AveragingCollector<isize>,
    channel3_battery_voltage: AveragingCollector<isize>,
    auxiliary_starter_voltage: AveragingCollector<isize>,
    battery_bank_midpoint_voltage: AveragingCollector<isize>,
    battery_bank_midpoint_deviation: AveragingCollector<isize>,
    panel_voltage: AveragingCollector<isize>,
    panel_power: AveragingCollector<isize>,
    main_battery_current: AveragingCollector<isize>,
    channel2_battery_current: AveragingCollector<isize>,
    channel3_battery_current: AveragingCollector<isize>,
    load_current: AveragingCollector<isize>,
    load_output_state: Option<String>,
    battery_temperature: AveragingCollector<isize>,
    instantaneous_power: AveragingCollector<isize>,
    consumed_amp_hours: Option<isize>,
    state_of_charge: Option<isize>,
    time_to_go: Option<isize>,
    alarm_active: Option<String>,
    relay_state: Option<String>,
    alarm_reason: Option<isize>,
    off_reason: Option<String>,
    discharge_depth_deepest: Option<isize>,
    discharge_depth_last: Option<isize>,
    discharge_depth_average: Option<isize>,
    number_of_charge_cycles: Option<isize>,
    number_of_full_discharges: Option<isize>,
    cumulative_amp_hours_drawn: Option<isize>,
    main_battery_voltage_min: Option<isize>,
    main_battery_voltage_max: Option<isize>,
    seconds_since_last_full_charge: Option<isize>,
    number_of_automatic_synchronizations: Option<isize>,
    number_of_alarms_main_voltage_low: Option<isize>,
    number_of_alarms_main_voltage_high: Option<isize>,
    number_of_alarms_auxiliary_voltage_low: Option<isize>,
    number_of_alarms_auxliary_voltage_high: Option<isize>,
    auxiliary_battery_voltage_min: Option<isize>,
    auxiliary_battery_voltage_max: Option<isize>,
    energy_discharged_or_produced: Option<f32>,
    energy_charged_or_consumed: Option<f32>,
    yield_total: Option<f32>,
    yield_today: Option<f32>,
    power_today_max: Option<isize>,
    yield_yesterday: Option<f32>,
    power_yesterday_max: Option<isize>,
    error_code: Option<isize>,
    state_of_operation: Option<isize>,
    firmware_version_16bit: Option<String>,
    firmware_version_24bit: Option<String>,
    product_id: Option<String>,
    serial_number: Option<String>,
    day_sequence_number: Option<isize>,
    device_mode: Option<isize>,
    ac_output_voltage: AveragingCollector<isize>,
    ac_output_current: AveragingCollector<isize>,
    ac_output_apparent_power: AveragingCollector<isize>,
    warning_reason: Option<isize>,
    tracker_mode_operation: Option<isize>,
    dc_monitor_mode: Option<isize>,

    last_collected_instant: Option<Instant>,
    main_battery_amp_hour_total: IntegratingCollector<f64>,

    message_count: usize,
    checksum_error_count: usize,
}

impl Collector {
    pub fn new() -> Collector {
        Collector {
            main_battery_amp_hour_total: IntegratingCollector {
                sum: 0.0,
                set: false,
                factor: MA_SEC_TO_MA_HOUR,
                clamp_max: Some(0.0),
                clamp_min: None,
            },
            ..Default::default()
        }
    }

    pub(crate) fn next(old: Collector) -> Collector {
        Collector {
            main_battery_amp_hour_total: old.main_battery_amp_hour_total,
            last_collected_instant: old.last_collected_instant,
            ..Default::default()
        }
    }

    pub fn increment_checksum_error(&mut self) {
        self.checksum_error_count += 1;
    }

    pub fn message_count(&self) -> usize {
        self.message_count
    }

    pub fn reset_totals(&mut self) {
        self.main_battery_amp_hour_total.reset();
    }

    pub(crate) fn collect(&mut self, msg: Message) {
        let old_state_of_op = self.state_of_operation;

        self.main_battery_voltage.collect(msg.main_battery_voltage);
        self.channel2_battery_voltage.collect(msg.channel2_battery_voltage);
        self.channel3_battery_voltage.collect(msg.channel3_battery_voltage);
        self.auxiliary_starter_voltage.collect(msg.auxiliary_starter_voltage);
        self.battery_bank_midpoint_voltage.collect(msg.battery_bank_midpoint_voltage);
        self.battery_bank_midpoint_deviation.collect(msg.battery_bank_midpoint_deviation);
        self.panel_voltage.collect(msg.panel_voltage);
        self.panel_power.collect(msg.panel_power);
        self.main_battery_current.collect(msg.main_battery_current);
        self.channel2_battery_current.collect(msg.channel2_battery_current);
        self.channel3_battery_current.collect(msg.channel3_battery_current);
        self.load_current.collect(msg.load_current);
        self.load_output_state = msg.load_output_state;
        self.battery_temperature.collect(msg.battery_temperature);
        self.instantaneous_power.collect(msg.instantaneous_power);
        self.consumed_amp_hours = msg.consumed_amp_hours;
        self.state_of_charge = msg.state_of_charge;
        self.time_to_go = msg.time_to_go;
        self.alarm_active = msg.alarm_active;
        self.relay_state = msg.relay_state;
        self.alarm_reason = msg.alarm_reason;
        self.off_reason = msg.off_reason;
        self.discharge_depth_deepest = msg.discharge_depth_deepest;
        self.discharge_depth_last = msg.discharge_depth_last;
        self.discharge_depth_average = msg.discharge_depth_average;
        self.number_of_charge_cycles = msg.number_of_charge_cycles;
        self.number_of_full_discharges = msg.number_of_full_discharges;
        self.cumulative_amp_hours_drawn = msg.cumulative_amp_hours_drawn;
        self.main_battery_voltage_min = msg.main_battery_voltage_min;
        self.main_battery_voltage_max = msg.main_battery_voltage_max;
        self.seconds_since_last_full_charge = msg.seconds_since_last_full_charge;
        self.number_of_automatic_synchronizations = msg.number_of_automatic_synchronizations;
        self.number_of_alarms_main_voltage_low = msg.number_of_alarms_main_voltage_low;
        self.number_of_alarms_main_voltage_high = msg.number_of_alarms_main_voltage_high;
        self.number_of_alarms_auxiliary_voltage_low = msg.number_of_alarms_auxiliary_voltage_low;
        self.number_of_alarms_auxliary_voltage_high = msg.number_of_alarms_auxliary_voltage_high;
        self.auxiliary_battery_voltage_min = msg.auxiliary_battery_voltage_min;
        self.auxiliary_battery_voltage_max = msg.auxiliary_battery_voltage_max;
        self.energy_discharged_or_produced = msg.energy_discharged_or_produced;
        self.energy_charged_or_consumed = msg.energy_charged_or_consumed;
        self.yield_total = msg.yield_total;
        self.yield_today = msg.yield_today;
        self.power_today_max = msg.power_today_max;
        self.yield_yesterday = msg.yield_yesterday;
        self.power_yesterday_max = msg.power_yesterday_max;
        self.error_code = msg.error_code;
        self.state_of_operation = msg.state_of_operation;
        self.firmware_version_16bit = msg.firmware_version_16bit;
        self.firmware_version_24bit = msg.firmware_version_24bit;
        self.product_id = msg.product_id;
        self.serial_number = msg.serial_number;
        self.day_sequence_number = msg.day_sequence_number;
        self.device_mode = msg.device_mode;
        self.ac_output_voltage.collect(msg.ac_output_voltage);
        self.ac_output_current.collect(msg.ac_output_current);
        self.ac_output_apparent_power.collect(msg.ac_output_apparent_power);
        self.warning_reason = msg.warning_reason;
        self.tracker_mode_operation = msg.tracker_mode_operation;
        self.dc_monitor_mode = msg.dc_monitor_mode;

        let now = Instant::now();
        if let Some(last_collected_instant) = self.last_collected_instant {
            let secs_since_last_collection = (now - last_collected_instant).as_secs_f64();
            if let Some(c) = msg.main_battery_current {
                let delta = c as f64 * secs_since_last_collection;
                self.main_battery_amp_hour_total.collect(Some(delta));
                // log::info!(
                //     "Collector: delta_time={}, current={}, delta={}, sum={}",
                //     secs_since_last_collection,
                //     c,
                //     delta,
                //     self.main_battery_amp_hour_total.sum
                // );
            }
        }
        self.last_collected_instant = Some(now);

        self.message_count += 1;

        let new_state_of_op = self.state_of_operation;

        if old_state_of_op.unwrap_or(-1) != StateOfOperation::Float as isize && new_state_of_op.unwrap_or(-1) == StateOfOperation::Float as isize {
            // Change int StateOfOperation::Float
            self.main_battery_amp_hour_total.sum = 0.0;
        }
    }

    pub fn build_json_message(&self) -> Value {
        let mut map = Map::new();

        let mm = &mut map;

        self.main_battery_voltage.insert_json(mm, "main_battery_voltage");
        self.channel2_battery_voltage.insert_json(mm, "channel2_battery_voltage");
        self.channel3_battery_voltage.insert_json(mm, "channel3_battery_voltage");
        self.auxiliary_starter_voltage.insert_json(mm, "auxiliary_starter_voltage");
        self.battery_bank_midpoint_voltage.insert_json(mm, "battery_bank_midpoint_voltage");
        self.battery_bank_midpoint_deviation.insert_json(mm, "battery_bank_midpoint_deviation");
        self.panel_voltage.insert_json(mm, "panel_voltage");
        self.panel_power.insert_json(mm, "panel_power");
        self.main_battery_current.insert_json(mm, "main_battery_current");
        self.channel2_battery_current.insert_json(mm, "channel2_battery_current");
        self.channel3_battery_current.insert_json(mm, "channel3_battery_current");
        self.load_current.insert_json(mm, "load_current");
        self.load_output_state.insert_json(mm, "load_output_state");
        self.battery_temperature.insert_json(mm, "battery_temperature");
        self.instantaneous_power.insert_json(mm, "instantaneous_power");
        self.consumed_amp_hours.insert_json(mm, "consumed_amp_hours");
        self.state_of_charge.insert_json(mm, "state_of_charge");
        self.time_to_go.insert_json(mm, "time_to_go");
        self.alarm_active.insert_json(mm, "alarm_active");
        self.relay_state.insert_json(mm, "relay_state");
        self.alarm_reason.insert_json(mm, "alarm_reason");
        self.off_reason.insert_json(mm, "off_reason");
        self.discharge_depth_deepest.insert_json(mm, "discharge_depth_deepest");
        self.discharge_depth_last.insert_json(mm, "discharge_depth_last");
        self.discharge_depth_average.insert_json(mm, "discharge_depth_average");
        self.number_of_charge_cycles.insert_json(mm, "number_of_charge_cycles");
        self.number_of_full_discharges.insert_json(mm, "number_of_full_discharges");
        self.cumulative_amp_hours_drawn.insert_json(mm, "cumulative_amp_hours_drawn");
        self.main_battery_voltage_min.insert_json(mm, "main_battery_voltage_min");
        self.main_battery_voltage_max.insert_json(mm, "main_battery_voltage_max");
        self.seconds_since_last_full_charge.insert_json(mm, "seconds_since_last_full_charge");
        self.number_of_automatic_synchronizations
            .insert_json(mm, "number_of_automatic_synchronizations");
        self.number_of_alarms_main_voltage_low.insert_json(mm, "number_of_alarms_main_voltage_low");
        self.number_of_alarms_main_voltage_high.insert_json(mm, "number_of_alarms_main_voltage_high");
        self.number_of_alarms_auxiliary_voltage_low
            .insert_json(mm, "number_of_alarms_auxiliary_voltage_low");
        self.number_of_alarms_auxliary_voltage_high
            .insert_json(mm, "number_of_alarms_auxliary_voltage_high");
        self.auxiliary_battery_voltage_min.insert_json(mm, "auxiliary_battery_voltage_min");
        self.auxiliary_battery_voltage_max.insert_json(mm, "auxiliary_battery_voltage_max");
        self.energy_discharged_or_produced.insert_json(mm, "energy_discharged_or_produced");
        self.energy_charged_or_consumed.insert_json(mm, "energy_charged_or_consumed");
        self.yield_total.insert_json(mm, "yield_total");
        self.yield_today.insert_json(mm, "yield_today");
        self.power_today_max.insert_json(mm, "power_today_max");
        self.yield_yesterday.insert_json(mm, "yield_yesterday");
        self.power_yesterday_max.insert_json(mm, "power_yesterday_max");
        self.error_code.insert_json(mm, "error_code");
        self.state_of_operation.insert_json(mm, "state_of_operation");
        self.firmware_version_16bit.insert_json(mm, "firmware_version_16bit");
        self.firmware_version_24bit.insert_json(mm, "firmware_version_24bit");
        self.product_id.insert_json(mm, "product_id");
        self.serial_number.insert_json(mm, "serial_number");
        self.day_sequence_number.insert_json(mm, "day_sequence_number");
        self.device_mode.insert_json(mm, "device_mode");
        self.ac_output_voltage.insert_json(mm, "ac_output_voltage");
        self.ac_output_current.insert_json(mm, "ac_output_current");
        self.ac_output_apparent_power.insert_json(mm, "ac_output_apparent_power");
        self.warning_reason.insert_json(mm, "warning_reason");
        self.tracker_mode_operation.insert_json(mm, "tracker_mode_operation");
        self.dc_monitor_mode.insert_json(mm, "dc_monitor_mode");

        self.main_battery_amp_hour_total.insert_json(mm, "main_battery_amp_hour_total");

        self.message_count.insert_json(mm, "message_count");
        self.checksum_error_count.insert_json(mm, "checksum_error_count");

        Value::Object(map)
    }
}

#[derive(Debug, Default, PartialEq)]
pub struct Message {
    pub main_battery_voltage: Option<isize>,
    pub channel2_battery_voltage: Option<isize>,
    pub channel3_battery_voltage: Option<isize>,
    pub auxiliary_starter_voltage: Option<isize>,
    pub battery_bank_midpoint_voltage: Option<isize>,
    pub battery_bank_midpoint_deviation: Option<isize>,
    pub panel_voltage: Option<isize>,
    pub panel_power: Option<isize>,
    pub main_battery_current: Option<isize>,
    pub channel2_battery_current: Option<isize>,
    pub channel3_battery_current: Option<isize>,
    pub load_current: Option<isize>,
    pub load_output_state: Option<String>,
    pub battery_temperature: Option<isize>,
    pub instantaneous_power: Option<isize>,
    pub consumed_amp_hours: Option<isize>,
    pub state_of_charge: Option<isize>,
    pub time_to_go: Option<isize>,
    pub alarm_active: Option<String>,
    pub relay_state: Option<String>,
    pub alarm_reason: Option<isize>,
    pub off_reason: Option<String>,
    pub discharge_depth_deepest: Option<isize>,
    pub discharge_depth_last: Option<isize>,
    pub discharge_depth_average: Option<isize>,
    pub number_of_charge_cycles: Option<isize>,
    pub number_of_full_discharges: Option<isize>,
    pub cumulative_amp_hours_drawn: Option<isize>,
    pub main_battery_voltage_min: Option<isize>,
    pub main_battery_voltage_max: Option<isize>,
    pub seconds_since_last_full_charge: Option<isize>,
    pub number_of_automatic_synchronizations: Option<isize>,
    pub number_of_alarms_main_voltage_low: Option<isize>,
    pub number_of_alarms_main_voltage_high: Option<isize>,
    pub number_of_alarms_auxiliary_voltage_low: Option<isize>,
    pub number_of_alarms_auxliary_voltage_high: Option<isize>,
    pub auxiliary_battery_voltage_min: Option<isize>,
    pub auxiliary_battery_voltage_max: Option<isize>,
    pub energy_discharged_or_produced: Option<f32>,
    pub energy_charged_or_consumed: Option<f32>,
    pub yield_total: Option<f32>,
    pub yield_today: Option<f32>,
    pub power_today_max: Option<isize>,
    pub yield_yesterday: Option<f32>,
    pub power_yesterday_max: Option<isize>,
    pub error_code: Option<isize>,
    pub state_of_operation: Option<isize>,
    pub firmware_version_16bit: Option<String>,
    pub firmware_version_24bit: Option<String>,
    pub product_id: Option<String>,
    pub serial_number: Option<String>,
    pub day_sequence_number: Option<isize>,
    pub device_mode: Option<isize>,
    pub ac_output_voltage: Option<isize>,
    pub ac_output_current: Option<isize>,
    pub ac_output_apparent_power: Option<isize>,
    pub warning_reason: Option<isize>,
    pub tracker_mode_operation: Option<isize>,
    pub dc_monitor_mode: Option<isize>,
}

impl Message {
    pub fn new() -> Message {
        Message::default()
    }

    pub fn set_field(&mut self, name: &str, value: &str) -> Result<(), String> {
        match name {
            "V" => self.main_battery_voltage = Some(as_int(value)?),
            "V2" => self.channel2_battery_voltage = Some(as_int(value)?),
            "V3" => self.channel3_battery_voltage = Some(as_int(value)?),
            "VS" => self.auxiliary_starter_voltage = Some(as_int(value)?),
            "VM" => self.battery_bank_midpoint_voltage = Some(as_int(value)?),
            "DM" => self.battery_bank_midpoint_deviation = Some(as_int(value)?),
            "VPV" => self.panel_voltage = Some(as_int(value)?),
            "PPV" => self.panel_power = Some(as_int(value)?),
            "I" => self.main_battery_current = Some(as_int(value)?),
            "I2" => self.channel2_battery_current = Some(as_int(value)?),
            "I3" => self.channel3_battery_current = Some(as_int(value)?),
            "IL" => self.load_current = Some(as_int(value)?),
            "LOAD" => self.load_output_state = Some(value.to_string()),
            "T" => self.battery_temperature = Some(as_int(value)?),
            "P" => self.instantaneous_power = Some(as_int(value)?),
            "CE" => self.consumed_amp_hours = Some(as_int(value)?),
            "SOC" => self.state_of_charge = Some(as_int(value)?),
            "TTG" => self.time_to_go = Some(as_int(value)?),
            "Alarm" => self.alarm_active = Some(value.to_string()),
            "Relay" => self.relay_state = Some(value.to_string()),
            "AR" => self.alarm_reason = Some(as_int(value)?),
            "OR" => self.off_reason = Some(value.to_string()),
            "H1" => self.discharge_depth_deepest = Some(as_int(value)?),
            "H2" => self.discharge_depth_last = Some(as_int(value)?),
            "H3" => self.discharge_depth_average = Some(as_int(value)?),
            "H4" => self.number_of_charge_cycles = Some(as_int(value)?),
            "H5" => self.number_of_full_discharges = Some(as_int(value)?),
            "H6" => self.cumulative_amp_hours_drawn = Some(as_int(value)?),
            "H7" => self.main_battery_voltage_min = Some(as_int(value)?),
            "H8" => self.main_battery_voltage_max = Some(as_int(value)?),
            "H9" => self.seconds_since_last_full_charge = Some(as_int(value)?),
            "H10" => self.number_of_automatic_synchronizations = Some(as_int(value)?),
            "H11" => self.number_of_alarms_main_voltage_low = Some(as_int(value)?),
            "H12" => self.number_of_alarms_main_voltage_high = Some(as_int(value)?),
            "H13" => self.number_of_alarms_auxiliary_voltage_low = Some(as_int(value)?),
            "H14" => self.number_of_alarms_auxliary_voltage_high = Some(as_int(value)?),
            "H15" => self.auxiliary_battery_voltage_min = Some(as_int(value)?),
            "H16" => self.auxiliary_battery_voltage_max = Some(as_int(value)?),
            "H17" => self.energy_discharged_or_produced = Some(as_float(value, 10.0)?),
            "H18" => self.energy_charged_or_consumed = Some(as_float(value, 10.0)?),
            "H19" => self.yield_total = Some(as_float(value, 10.0)?),
            "H20" => self.yield_today = Some(as_float(value, 10.0)?),
            "H21" => self.power_today_max = Some(as_int(value)?),
            "H22" => self.yield_yesterday = Some(as_float(value, 10.0)?),
            "H23" => self.power_yesterday_max = Some(as_int(value)?),
            "ERR" => self.error_code = Some(as_int(value)?),
            "CS" => self.state_of_operation = Some(as_int(value)?),
            // "BMV" => self.model_description = Some(value.to_string(),
            "FW" => self.firmware_version_16bit = Some(value.to_string()),
            "FWE" => self.firmware_version_24bit = Some(value.to_string()),
            "PID" => self.product_id = Some(value.to_string()),
            "SER#" => self.serial_number = Some(value.to_string()),
            "HSDS" => self.day_sequence_number = Some(as_int(value)?),
            "MODE" => self.device_mode = Some(as_int(value)?),
            "AC_OUT_V" => self.ac_output_voltage = Some(as_int(value)?),
            "AC_OUT_I" => self.ac_output_current = Some(as_int(value)?),
            "AC_OUT_S" => self.ac_output_apparent_power = Some(as_int(value)?),
            "WARN" => self.warning_reason = Some(as_int(value)?),
            "MPPT" => self.tracker_mode_operation = Some(as_int(value)?),
            "MON" => self.dc_monitor_mode = Some(as_int(value)?),
            other => return Err(format!("Unhandled field '{}'", other)),
        };
        Ok(())
    }
}

fn as_int(value: &str) -> Result<isize, String> {
    value.parse::<isize>().map_err(|e| format!("Invalid integer {}", e))
}

fn as_float(value: &str, factor: f32) -> Result<f32, String> {
    value.parse::<f32>().map(|v| v * factor).map_err(|e| format!("Invalid float {}", e))
}

#[allow(dead_code)]
enum StateOfOperation {
    Off = 0,
    LowPower = 1,
    Fault = 2,
    Bulk = 3,
    Absorption = 4,
    Float = 5,
    Storage = 6,
    EqualizeManual = 7,
    Inverting = 9,
    PowerSupply = 11,
    StartingUp = 245,
    RepeatedAbsorption = 246,
    AutoEqualize = 247,
    BatterySafe = 248,
    ExternalControl = 252,
}
