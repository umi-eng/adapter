set -e

while true
do
    # standard frames
    cansend can0 123#DEADBEEF
    cansend can1 321#BEEFDEAD
    # extended frames.
    cansend can0 1F334455#1122334455667788
    cansend can1 1F334455#5566778811223344
    # todo: canfd frames.
    cansend can0 213##311223344
    cansend can1 231##311223344

    sleep 0.010
done
