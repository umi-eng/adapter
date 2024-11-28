# Only runs on Linux.
# Must be run as root on most systems.

# Exit if anything returns an error.
set -e

echo "Testing standard bitrates."

echo "   5 Mbit/s"; ip link set can0 type can bitrate 5000000
echo "   4 Mbit/s"; ip link set can0 type can bitrate 4000000
echo "   3 Mbit/s"; ip link set can0 type can bitrate 3000000
echo "   2 Mbit/s"; ip link set can0 type can bitrate 2000000
echo "   1 Mbit/s"; ip link set can0 type can bitrate 1000000
echo " 500 kbit/s"; ip link set can0 type can bitrate 500000
echo " 250 kbit/s"; ip link set can0 type can bitrate 250000
echo " 125 kbit/s"; ip link set can0 type can bitrate 125000
echo "  50 kbit/s"; ip link set can0 type can bitrate 50000
echo "  25 kbit/s"; ip link set can0 type can bitrate 25000
echo "12.5 kbit/s"; ip link set can0 type can bitrate 12500


echo "Testing FD bitrates."

echo "nominal = 5 Mbit/s, data = 5 Mbit/s"
ip link set can0 type can bitrate 5000000 dbitrate 5000000 fd on
echo "nominal = 250 kbit/s, data = 1 Mbit/s"
ip link set can0 type can bitrate 250000 dbitrate 100000 fd on
echo "nominal = 12.5 kbit/s, data = 1 Mbit/s"
ip link set can0 type can bitrate 12500 dbitrate 100000 fd on


echo "Testing sample points."

echo "87.5%"; ip link set can0 type can bitrate 250000 sample-point 0.875
echo "75.0%"; ip link set can0 type can bitrate 250000 sample-point 0.750
echo "50.0%"; ip link set can0 type can bitrate 250000 sample-point 0.500


echo "Finished..."
