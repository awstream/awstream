## Test configuration 1280 0 20
w <- 1280
s <- 0
q <- 20
dir <- "/Users/benzh/box/AdaptiveStream/darknet-test-output"

bw.f <- sprintf("%s/bw-%dx%dx%d.csv", dir, w, s, q)
bw <- read.csv(bw.f, header=F);
bw <- head(tail(bw, -1), 49)
names(bw) <- c("time", "bandwidth")
bw$time <- bw$time * 5
bw.ref <- 9.81

bw.plot <- ggplot(bw, aes(x=time, y=bandwidth)) +
    geom_line() +
    xlab("") +
    ylab("Bandwidth (mbps)") +
    geom_hline(yintercept=bw.ref, linetype="dotdash") +
    academic_paper_theme()
bw.plot

acc.f <- sprintf("%s/acc-%dx%dx%d.csv", dir, w, s, q)
acc <- read.csv(f, header=F);
acc <- head(tail(acc, -1), 49)
names(acc) <- c("time", "accuracy")
acc$time <- acc$time * 5
acc.ref <- 0.90

acc.plot <- ggplot(acc, aes(x=time, y=accuracy)) +
    geom_line() +
    xlab("time (seconds)") +
    ylab("Accuracy (F1, %)") +
    ylim(0, 1) +
    geom_hline(yintercept=acc.ref, linetype="dotdash") +
    academic_paper_theme()

pdf("online.motiv.pdf", width=4, height=5)
multiplot(bw.plot, acc.plot, cols=1)
dev.off()
