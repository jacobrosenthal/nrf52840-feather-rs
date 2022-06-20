use core::cell::RefCell;

use defmt::{info, unwrap};
use embassy::blocking_mutex::raw::ThreadModeRawMutex;
use embassy::blocking_mutex::ThreadModeMutex;
use embassy::channel::mpmc::Sender;
use embassy::util::{select, Either};
use embassy_nrf::gpio::{self, Pin};
use embassy_nrf::gpiote::{self, Channel as _};
use nrf_softdevice::ble::{gatt_server, peripheral};
use nrf_softdevice::{raw, Softdevice};

#[nrf_softdevice::gatt_server]
pub struct Server {
    pub text: TextService,
    pub battery: BatteryService,
}

#[nrf_softdevice::gatt_service(uuid = "9e7312e0-2354-11eb-9f10-fbc30a62cf38")]
pub struct TextService {
    #[characteristic(uuid = "9e7312e0-2354-11eb-9f10-fbc30a63cf38", read, write)]
    pub message: heapless::String<20>,
}

#[nrf_softdevice::gatt_service(uuid = "180f")]
pub struct BatteryService {
    #[characteristic(uuid = "2a19", read, notify)]
    pub percentage: u8,
}

// tasks in same executor guaranteed to not be running at same time as they cant interupt eachother
pub static SERVER: ThreadModeMutex<RefCell<Option<Server>>> =
    ThreadModeMutex::new(RefCell::new(None));

#[embassy::task]
pub async fn bluetooth_task(
    sd: &'static Softdevice,
    channel: Sender<'static, ThreadModeRawMutex, bool, 1>,
    percent: u8,
) {
    let dp = unsafe { <embassy_nrf::Peripherals as embassy::util::Steal>::steal() };

    let server: Server = unwrap!(gatt_server::register(sd));

    // services start uninitialized
    unwrap!(server
        .text
        .message_set(heapless::String::from("                    ")));
    unwrap!(server.battery.percentage_set(percent));

    // going to share these with multiple futures which will be created and
    // destroyed complicating lifetimes otherwise
    SERVER.borrow().replace(Some(server));

    // button presses will be delivered on HiToLo or when you release the button
    let button1 = gpiote::InputChannel::new(
        // degrade just a typesystem hack to forget which pin it is so we can
        // call it Anypin and make our function calls more generic
        dp.GPIOTE_CH1.degrade(),
        gpio::Input::new(dp.P1_02.degrade(), gpio::Pull::Up),
        gpiote::InputChannelPolarity::HiToLo,
    );

    #[rustfmt::skip]
    let adv_data = &[
        0x02, 0x01, raw::BLE_GAP_ADV_FLAGS_LE_ONLY_GENERAL_DISC_MODE as u8,
        0x03, 0x03, 0x09, 0x18,
        0x0a, 0x09, b'H', b'e', b'l', b'l', b'o', b'R', b'u', b's', b't',
    ];

    #[rustfmt::skip]
    let scan_data = &[
        0x03, 0x03, 0x09, 0x18,
    ];

    let config = peripheral::Config::default();

    'waiting: loop {
        info!("Bluetooth is OFF");
        info!("Press button to enable, press again to disconnect");

        // wait here until button is pressed
        button1.wait().await;

        loop {
            info!("advertising!");

            let adv = peripheral::ConnectableAdvertisement::ScannableUndirected {
                adv_data,
                scan_data,
            };

            let conn_fut = peripheral::advertise_connectable(sd, adv, &config);

            let conn = match select(conn_fut, button1.wait()).await {
                // button returns if pressed and stops advertising
                Either::First(conn) => unwrap!(conn),
                Either::Second(_) => {
                    info!("waiting!");
                    continue 'waiting;
                }
            };

            info!("connected!");

            if let Some(server) = SERVER.borrow().borrow().as_ref() {
                // Run the GATT server on the connection. This returns when the connection gets disconnected.
                let gatt_future = gatt_server::run(&conn, server, |e| match e {
                    ServerEvent::Battery(e) => match e {
                        BatteryServiceEvent::PercentageCccdWrite { notifications } => {
                            info!("battery notifications: {}", notifications)
                        }
                    },
                    ServerEvent::Text(e) => match e {
                        TextServiceEvent::MessageWrite(val) => {
                            defmt::info!("ble recv {}", val.as_bytes());
                            channel.try_send(true).ok();
                        }
                    },
                });

                // turn off ble after button or when disconnect
                match select(gatt_future, button1.wait()).await {
                    Either::First(_) => continue 'waiting,
                    Either::Second(_) => continue 'waiting,
                }
            }
        }
    }
}

#[embassy::task]
pub async fn softdevice_task(sd: &'static Softdevice) {
    sd.run().await;
}

pub fn softdevice_config() -> nrf_softdevice::Config {
    nrf_softdevice::Config {
        clock: Some(raw::nrf_clock_lf_cfg_t {
            source: raw::NRF_CLOCK_LF_SRC_XTAL as u8,
            rc_ctiv: 0,
            rc_temp_ctiv: 0,
            accuracy: raw::NRF_CLOCK_LF_ACCURACY_20_PPM as u8,
        }),
        conn_gap: Some(raw::ble_gap_conn_cfg_t {
            conn_count: 6,
            event_length: 24,
        }),
        conn_gatt: Some(raw::ble_gatt_conn_cfg_t { att_mtu: 256 }),
        gatts_attr_tab_size: Some(raw::ble_gatts_cfg_attr_tab_size_t {
            attr_tab_size: 32768,
        }),
        gap_role_count: Some(raw::ble_gap_cfg_role_count_t {
            adv_set_count: 1,
            periph_role_count: 3,
            central_role_count: 3,
            central_sec_count: 0,
            _bitfield_1: raw::ble_gap_cfg_role_count_t::new_bitfield_1(0),
        }),
        gap_device_name: Some(raw::ble_gap_cfg_device_name_t {
            p_value: b"HelloRust" as *const u8 as _,
            current_len: 9,
            max_len: 9,
            write_perm: unsafe { core::mem::zeroed() },
            _bitfield_1: raw::ble_gap_cfg_device_name_t::new_bitfield_1(
                raw::BLE_GATTS_VLOC_STACK as u8,
            ),
        }),
        ..Default::default()
    }
}
