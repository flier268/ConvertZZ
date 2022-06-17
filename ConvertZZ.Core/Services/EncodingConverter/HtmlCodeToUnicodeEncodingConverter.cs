using System.Text;

namespace ConvertZZ.Core.Services.EncodingConverter
{
    public class HtmlCodeToUnicodeEncodingConverter : CommonEncodingConverter
    {
        public override string Convert(string str, Encoding originEncoding, Encoding targetEncoding)
        {
            return Convert(str);
        }

        public string Convert(string str)
        {
            StringBuilder sb = new();
            str.Replace("&amp;", "&");
            str.Replace("&lt;", "<");
            str.Replace("&gt;", ">");
            //以;將文字拆成陣列
            string[] tmp = str.Split(';');
            //檢查最後一個字元是否為【;】，因為有【英文】、【阿拉伯數字】、【&#XXXX;】
            //若最後一個要處理的字並非HTML UNICODE則不進行處理
            bool Process_last = str.Substring(str.Length - 1, 1).Equals(";");
            //Debug.WriteLine(tmp.Length + "");
            for (int i = 0; i < tmp.Length; i++)
            {
                //以&#將文字拆成陣列
                string[] tmp2 = tmp[i].Split(new string[] { "&#" }, StringSplitOptions.RemoveEmptyEntries);
                if (tmp2.Length == 1)
                {
                    //如果長度為1則試圖轉換UNICODE回字符，若失敗則使用原本的字元
                    if (i != tmp.Length - 1)
                    {
                        try
                        {
                            if (tmp2[0].StartsWith("x"))
                                sb.Append(System.Convert.ToChar(System.Convert.ToInt32(tmp2[0].Substring(1, tmp2[0].Length - 1), 16)).ToString());
                            else
                                sb.Append(System.Convert.ToChar(System.Convert.ToInt32(int.Parse(tmp2[0]))).ToString());
                        }
                        catch
                        {
                            sb.Append(tmp2[0]);
                        }
                    }
                    else
                    {
                        sb.Append(tmp2[0]);
                    }
                }
                if (tmp2.Length == 2)
                {
                    //若長度為2，則第一項不處理，只處理第二項即可
                    sb.Append(tmp2[0]);
                    var g = System.Convert.ToInt32(tmp2[1].Substring(1, tmp2[1].Length - 1), 16);
                    if (tmp2[1].StartsWith("x"))
                        sb.Append(System.Convert.ToChar(System.Convert.ToInt32(tmp2[1].Substring(1, tmp2[1].Length - 1), 16)).ToString());
                    else
                        sb.Append(System.Convert.ToChar(System.Convert.ToInt32(tmp2[1])).ToString());
                }
            }
            return sb.ToString();
        }
    }
}