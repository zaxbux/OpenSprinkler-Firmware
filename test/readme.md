# ConfigFS GPIO Simulation

## Setup

### Mount the configuration filesystem

```
mkdir -p /config
mount -t configfs none /config
```

### Create the device

```
mkdir /config/gpio-sim/gpio-device
mkdir /config/gpio-sim/gpio-device/gpio-bank0
echo "pinctrl-bcm2835" > /config/gpio-sim/gpio-device/gpio-bank0/label
```

### Configure the chip

```
echo 58 > /config/gpio-sim/gpio-device/gpio-bank0/num_lines
echo "ID_SDA" > /config/gpio-sim/gpio-device/gpio-bank0/line0/name
echo "ID_SCL" > /config/gpio-sim/gpio-device/gpio-bank0/line1/name
echo "SDA1" > /config/gpio-sim/gpio-device/gpio-bank0/line2/name
echo "SCL1" > /config/gpio-sim/gpio-device/gpio-bank0/line3/name
echo "GPIO_GCLK" > /config/gpio-sim/gpio-device/gpio-bank0/line4/name
echo "GPIO5" > /config/gpio-sim/gpio-device/gpio-bank0/line5/name
echo "GPIO6" > /config/gpio-sim/gpio-device/gpio-bank0/line6/name
echo "SPI_CE1_N" > /config/gpio-sim/gpio-device/gpio-bank0/line7/name
echo "SPI_CE0_N" > /config/gpio-sim/gpio-device/gpio-bank0/line8/name
echo "SPI_MISO" > /config/gpio-sim/gpio-device/gpio-bank0/line9/name
echo "SPI_MOSI" > /config/gpio-sim/gpio-device/gpio-bank0/line10/name
echo "SPI_SCLK" > /config/gpio-sim/gpio-device/gpio-bank0/line11/name
echo "GPIO12" > /config/gpio-sim/gpio-device/gpio-bank0/line12/name
echo "GPIO13" > /config/gpio-sim/gpio-device/gpio-bank0/line13/name
echo "TXD1" > /config/gpio-sim/gpio-device/gpio-bank0/line14/name
echo "RXD1" > /config/gpio-sim/gpio-device/gpio-bank0/line15/name
echo "GPIO16" > /config/gpio-sim/gpio-device/gpio-bank0/line16/name
echo "GPIO17" > /config/gpio-sim/gpio-device/gpio-bank0/line17/name
echo "GPIO18" > /config/gpio-sim/gpio-device/gpio-bank0/line18/name
echo "GPIO19" > /config/gpio-sim/gpio-device/gpio-bank0/line19/name
echo "GPIO20" > /config/gpio-sim/gpio-device/gpio-bank0/line20/name
echo "GPIO21" > /config/gpio-sim/gpio-device/gpio-bank0/line21/name
echo "GPIO22" > /config/gpio-sim/gpio-device/gpio-bank0/line22/name
echo "GPIO23" > /config/gpio-sim/gpio-device/gpio-bank0/line23/name
echo "GPIO24" > /config/gpio-sim/gpio-device/gpio-bank0/line24/name
echo "GPIO25" > /config/gpio-sim/gpio-device/gpio-bank0/line25/name
echo "GPIO26" > /config/gpio-sim/gpio-device/gpio-bank0/line26/name
echo "GPIO27" > /config/gpio-sim/gpio-device/gpio-bank0/line27/name
echo "RGMII_MDIO" > /config/gpio-sim/gpio-device/gpio-bank0/line28/name
echo "RGMIO_MDC" > /config/gpio-sim/gpio-device/gpio-bank0/line29/name
echo "CTS0" > /config/gpio-sim/gpio-device/gpio-bank0/line30/name
echo "RTS0" > /config/gpio-sim/gpio-device/gpio-bank0/line31/name
echo "TXD0" > /config/gpio-sim/gpio-device/gpio-bank0/line32/name
echo "RXD0" > /config/gpio-sim/gpio-device/gpio-bank0/line33/name
echo "SD1_CLK" > /config/gpio-sim/gpio-device/gpio-bank0/line34/name
echo "SD1_CMD" > /config/gpio-sim/gpio-device/gpio-bank0/line35/name
echo "SD1_DATA0" > /config/gpio-sim/gpio-device/gpio-bank0/line36/name
echo "SD1_DATA1" > /config/gpio-sim/gpio-device/gpio-bank0/line37/name
echo "SD1_DATA2" > /config/gpio-sim/gpio-device/gpio-bank0/line38/name
echo "SD1_DATA3" > /config/gpio-sim/gpio-device/gpio-bank0/line39/name
echo "PWM0_MISO" > /config/gpio-sim/gpio-device/gpio-bank0/line40/name
echo "PWM1_MOSI" > /config/gpio-sim/gpio-device/gpio-bank0/line41/name
echo "STATUS_LED_G_CLK" > /config/gpio-sim/gpio-device/gpio-bank0/line42/name
echo "SPIFLASH_CE_N" > /config/gpio-sim/gpio-device/gpio-bank0/line43/name
echo "SDA0" > /config/gpio-sim/gpio-device/gpio-bank0/line44/name
echo "SCL0" > /config/gpio-sim/gpio-device/gpio-bank0/line45/name
echo "RGMII_RXCLK" > /config/gpio-sim/gpio-device/gpio-bank0/line46/name
echo "RGMII_RXCTL" > /config/gpio-sim/gpio-device/gpio-bank0/line47/name
echo "RGMII_RXD0" > /config/gpio-sim/gpio-device/gpio-bank0/line48/name
echo "RGMII_RXD1" > /config/gpio-sim/gpio-device/gpio-bank0/line49/name
echo "RGMII_RXD2" > /config/gpio-sim/gpio-device/gpio-bank0/line50/name
echo "RGMII_RXD3" > /config/gpio-sim/gpio-device/gpio-bank0/line51/name
echo "RGMII_TXCLK" > /config/gpio-sim/gpio-device/gpio-bank0/line52/name
echo "RGMII_TXCTL" > /config/gpio-sim/gpio-device/gpio-bank0/line53/name
echo "RGMII_TXD0" > /config/gpio-sim/gpio-device/gpio-bank0/line54/name
echo "RGMII_TXD1" > /config/gpio-sim/gpio-device/gpio-bank0/line55/name
echo "RGMII_TXD2" > /config/gpio-sim/gpio-device/gpio-bank0/line56/name
echo "RGMII_TXD3" > /config/gpio-sim/gpio-device/gpio-bank0/line57/name
```

### Activate the device

```
echo 1 > /config/gpio-sim/gpio-device/live
```