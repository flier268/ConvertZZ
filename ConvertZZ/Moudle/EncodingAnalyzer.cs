//部分參考自https://github.com/darkthread/CEAD/blob/master/ChEncAutoDetector/ChEncAutoDetector.cs
using System.Linq;
using System.Text;

namespace ConvertZZ.Moudle
{
    class EncodingAnalyzer
    {
        /// <summary>
        /// 快速篩選string是BIG5或GBK
        /// </summary>
        /// <param name="String"></param>
        /// <returns>
        /// -1: ASCII
        /// 0: BIG5
        /// 1: BIG5格式的GBK，建議使用BIG5->GBK
        /// 2: GBK
        /// 3: GBK格式的BIG5，建議使用GBK->BIG5
        /// </returns>
        public static int Analyze(string String)
        {
            Encoding BIG5 = Encoding.GetEncoding("BIG5");
            Encoding GBK = Encoding.GetEncoding("GBK");
            float[] report = new float[4];
            report[0] = AnalyzeBig5(BIG5.GetBytes(String)).BadSmell;
            report[1] = AnalyzeBig5(GBK.GetBytes(String)).BadSmell;
            report[2] = AnalyzeGBK(BIG5.GetBytes(String)).BadSmell;
            report[3] = AnalyzeGBK(GBK.GetBytes(String)).BadSmell;
            if (report.Sum() == 0)
                return -1;
            for (int i = 0; i < report.Length; i++)
                if (report[i] == report.Min())
                    return i;
            return 0;
        }

        /// <summary>
        /// 分析報告
        /// </summary>
        class Report
        {
            //統計解讀為ASCII、符號、常用字、次常用字及亂碼(非有效字元)字數
            public int Ascii, Symbol, Common, Rare, Unknow;
            /// <summary>
            /// 亂碼指標(數值愈大，不是該編碼的機率愈高)
            /// </summary>
            public float BadSmell
            {
                get
                {
                    int total = Ascii + Symbol + Common + Rare + Unknow;
                    if (total == 0) return 0;
                    return (float)(Rare + Unknow * 3) / total;
                }
            }
        }
        private static Report AnalyzeBig5(byte[] data)
        {
            Report res = new Report();
            bool isDblBytes = false;
            byte dblByteHi = 0;
            foreach (byte b in data)
            {
                if (isDblBytes)
                {
                    if (b >= 0x40 && b <= 0x7e || b >= 0xa1 && b <= 0xfe)
                    {
                        int c = dblByteHi * 0x100 + b;
                        if (c >= 0xa140 && c <= 0xa3bf)
                            res.Symbol++; //符號
                        else if (c >= 0xa440 && c <= 0xc67e)
                            res.Common++; //常用字
                        else if (c >= 0xc940 && c <= 0xf9d5)
                            res.Rare++; //次常用字
                        else
                            res.Unknow++; //無效字元
                    }
                    else
                        res.Unknow++;
                    isDblBytes = false;
                }
                else if (b >= 0x80 && b <= 0xfe)
                {
                    isDblBytes = true;
                    dblByteHi = b;
                }
                else if (b < 0x80)
                    res.Ascii++;
            }
            return res;
        }
        private static Report AnalyzeGBK(byte[] data)
        {
            Report res = new Report();
            bool isDblBytes = false;
            byte dblByteHi = 0;
            foreach (byte b in data)
            {
                if (isDblBytes)
                {
                    if (b >= 0xa1 && b <= 0xfe)
                    {
                        if (dblByteHi >= 0xa1 && dblByteHi <= 0xa9)
                            res.Symbol++; //符號
                        else if (dblByteHi >= 0xb0 && dblByteHi <= 0xd7)
                            res.Common++; //一級漢字(常用字)
                        else if (dblByteHi >= 0xd8 && dblByteHi <= 0xf7)
                            res.Rare++; //二級漢字(次常用字)
                        else
                            res.Unknow++; //無效字元
                    }
                    else
                        res.Unknow++; //無效字元
                    isDblBytes = false;
                }
                else if (b >= 0xa1 && b <= 0xf7)
                {
                    isDblBytes = true;
                    dblByteHi = b;
                }
                else if (b < 0x80)
                    res.Ascii++;
            }
            return res;
        }
    }
}
