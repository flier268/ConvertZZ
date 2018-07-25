using System;
using System.Text;

namespace ConvertZZ
{
    public class TransferEncodeHelper
    {
        public static string TransferStr(string str, Encoding originalEncode, Encoding targetEncode)
        {
            try
            {
                byte[] temp = originalEncode.GetBytes(str);
                byte[] targetEncodeBytes = Encoding.Convert(originalEncode, targetEncode, temp);

                char[] targetEncodeChars = new char[targetEncode.GetCharCount(targetEncodeBytes, 0, targetEncodeBytes.Length)];
                targetEncode.GetChars(targetEncodeBytes, 0, targetEncodeBytes.Length, targetEncodeChars, 0);
                string targetEncodeString = new string(targetEncodeChars);
                return targetEncodeString;
            }
            catch
            {
                Console.WriteLine("There is an exception.");
                return null;
            }
        }
    }
}
